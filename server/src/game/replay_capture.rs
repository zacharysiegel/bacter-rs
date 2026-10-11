use std::path::{Path, PathBuf};

use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;

use shared::error::AppError;
use shared::game::GameId;
use shared::protocol::{InputBundleSerialOut, ReplayHeaderSerialOut};
use shared::replay;

use crate::game::game_runtime;

pub const REPLAY_CAPTURE_CHANNEL_CAPACITY: usize = 1024;
const REPLAY_FILE_EXTENSION: &str = "replay";

/// Appends a game's replay records to its replay file from a writer task.
pub struct ReplayCapture {
    game_id: GameId,
    /// None once the capture is abandoned.
    records: Option<mpsc::Sender<ReplayRecord>>,
}

impl ReplayCapture {
    pub fn start(
        directory: &Path,
        game_id: GameId,
        header_serial_out: ReplayHeaderSerialOut,
    ) -> (ReplayCapture, JoinHandle<()>) {
        let (record_sender, record_receiver): (mpsc::Sender<ReplayRecord>, mpsc::Receiver<ReplayRecord>) =
            mpsc::channel(REPLAY_CAPTURE_CHANNEL_CAPACITY);
        let path: PathBuf = get_replay_path(directory, game_id);

        let replay_capture: ReplayCapture =
            ReplayCapture::from_record_sender(game_id, record_sender, header_serial_out);

        let writer_handle: JoinHandle<()> =
            game_runtime::GAME_RUNTIME.spawn(run_replay_writer(game_id, path, record_receiver));

        (replay_capture, writer_handle)
    }

    pub fn from_record_sender(
        game_id: GameId,
        record_sender: mpsc::Sender<ReplayRecord>,
        header_serial_out: ReplayHeaderSerialOut,
    ) -> ReplayCapture {
        let mut replay_capture: ReplayCapture = ReplayCapture {
            game_id,
            records: Some(record_sender),
        };

        replay_capture.send(ReplayRecord::Header(header_serial_out));

        replay_capture
    }

    pub fn record_bundle(&mut self, bundle_serial_out: InputBundleSerialOut) {
        self.send(ReplayRecord::Bundle(bundle_serial_out));
    }

    /// Closes the channel; the writer task ends once it has written every record sent before.
    pub fn finish(self) {
        drop(self.records);
    }

    /// A full or closed channel abandons the capture.
    fn send(&mut self, record: ReplayRecord) {
        let Some(records) = &self.records else {
            return;
        };

        let send_result: Result<(), TrySendError<ReplayRecord>> = records.try_send(record);

        if send_result.is_err() {
            log::warn!("replay capture abandoned; [game_id={}]", self.game_id.0);
            self.records = None;
        }
    }
}

pub fn get_replay_path(directory: &Path, game_id: GameId) -> PathBuf {
    directory.join(game_id.0.to_string()).with_extension(REPLAY_FILE_EXTENSION)
}

pub enum ReplayRecord {
    Header(ReplayHeaderSerialOut),
    Bundle(InputBundleSerialOut),
}

async fn run_replay_writer(game_id: GameId, path: PathBuf, record_receiver: mpsc::Receiver<ReplayRecord>) {
    let write_result: Result<(), AppError> = write_replay_records(&path, record_receiver).await;

    if let Err(error) = write_result {
        log::error!("replay capture failed; [game_id={} error={error}]", game_id.0);
    }
}

async fn write_replay_records(path: &Path, mut record_receiver: mpsc::Receiver<ReplayRecord>) -> Result<(), AppError> {
    if let Some(directory) = path.parent() {
        tokio::fs::create_dir_all(directory).await?;
    }

    let mut file: File = File::create(path).await?;
    file.write_all(&replay::get_version_prefix()).await?;

    while let Some(record) = record_receiver.recv().await {
        let record_bytes: Vec<u8> = match &record {
            ReplayRecord::Header(header_serial_out) => replay::get_header_record_bytes(header_serial_out),
            ReplayRecord::Bundle(bundle_serial_out) => replay::get_bundle_record_bytes(bundle_serial_out),
        };

        file.write_all(&record_bytes).await?;
        // The file is a valid replay prefix at every moment.
        file.flush().await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameModeKind, GameSettings, InputBundle, MemberEvent, Tick};
    use shared::member::MemberId;
    use shared::protocol::GameSettingsSerialOut;
    use shared::replay::{ReplayHeader, ReplayLog};

    fn create_bundle(tick: u32) -> InputBundle {
        InputBundle {
            tick: Tick(tick),
            member_events: vec![MemberEvent::Left {
                member_id: MemberId(tick),
            }],
            player_inputs: Vec::new(),
        }
    }

    #[test]
    fn start_writes_the_header_and_every_recorded_bundle() {
        let directory: PathBuf = test_fixture::create_temporary_directory("replay-capture");
        let settings: GameSettings = test_fixture::create_settings(GameModeKind::FreeForAll, 8);
        let header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut {
            settings: GameSettingsSerialOut::from(&settings),
            seed: 77,
        };

        let (mut replay_capture, writer_handle): (ReplayCapture, JoinHandle<()>) =
            ReplayCapture::start(&directory.join("nested"), GameId(5), header_serial_out);
        replay_capture.record_bundle(InputBundleSerialOut::from(&create_bundle(1)));
        replay_capture.record_bundle(InputBundleSerialOut::from(&create_bundle(2)));
        replay_capture.finish();
        game_runtime::GAME_RUNTIME.block_on(writer_handle).unwrap();

        let replay_bytes: Vec<u8> = std::fs::read(directory.join("nested/5.replay")).unwrap();

        assert_eq!(
            replay::read_replay(&replay_bytes).unwrap(),
            ReplayLog {
                header: ReplayHeader { settings, seed: 77 },
                input_bundles: vec![create_bundle(1), create_bundle(2)],
            },
        );
    }

    #[test]
    fn record_bundle_abandons_the_capture_when_the_channel_is_full() {
        let directory: PathBuf = test_fixture::create_temporary_directory("replay-capture-full");
        let settings: GameSettings = test_fixture::create_settings(GameModeKind::FreeForAll, 8);
        let header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut {
            settings: GameSettingsSerialOut::from(&settings),
            seed: 77,
        };
        let (record_sender, record_receiver): (mpsc::Sender<ReplayRecord>, mpsc::Receiver<ReplayRecord>) =
            mpsc::channel(2);

        let mut replay_capture: ReplayCapture =
            ReplayCapture::from_record_sender(GameId(6), record_sender, header_serial_out);
        replay_capture.record_bundle(InputBundleSerialOut::from(&create_bundle(1)));
        replay_capture.record_bundle(InputBundleSerialOut::from(&create_bundle(2)));
        replay_capture.record_bundle(InputBundleSerialOut::from(&create_bundle(3)));

        assert!(replay_capture.records.is_none());

        let path: PathBuf = directory.join("6.replay");
        let writer_handle: JoinHandle<()> =
            game_runtime::GAME_RUNTIME.spawn(run_replay_writer(GameId(6), path.clone(), record_receiver));
        replay_capture.finish();
        game_runtime::GAME_RUNTIME.block_on(writer_handle).unwrap();

        let replay_bytes: Vec<u8> = std::fs::read(&path).unwrap();

        assert_eq!(
            replay::read_replay(&replay_bytes).unwrap(),
            ReplayLog {
                header: ReplayHeader { settings, seed: 77 },
                input_bundles: vec![create_bundle(1)],
            },
        );
    }
}
