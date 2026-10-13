#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageShowKind {
    RestoredFromBackForwardCache,
    Loaded,
}

impl PageShowKind {
    /// `persisted` as `PageTransitionEvent.persisted` gives it.
    pub fn from_persisted(persisted: bool) -> PageShowKind {
        if persisted {
            PageShowKind::RestoredFromBackForwardCache
        } else {
            PageShowKind::Loaded
        }
    }

    pub fn get_effect(self, page_hide_closure: PageHideClosureKind) -> PageShowEffectKind {
        match (self, page_hide_closure) {
            (PageShowKind::RestoredFromBackForwardCache, PageHideClosureKind::SocketClosed) => {
                PageShowEffectKind::ConnectionLost
            }
            (PageShowKind::RestoredFromBackForwardCache, PageHideClosureKind::NoSocket)
            | (PageShowKind::Loaded, PageHideClosureKind::SocketClosed | PageHideClosureKind::NoSocket) => {
                PageShowEffectKind::Unchanged
            }
        }
    }
}

/// What the last `pagehide` did to the game socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageHideClosureKind {
    SocketClosed,
    NoSocket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageShowEffectKind {
    ConnectionLost,
    Unchanged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_persisted_is_a_restore_only_for_a_page_from_the_back_forward_cache() {
        assert_eq!(
            PageShowKind::from_persisted(true),
            PageShowKind::RestoredFromBackForwardCache,
        );
        assert_eq!(PageShowKind::from_persisted(false), PageShowKind::Loaded);
    }

    #[test]
    fn get_effect_loses_the_connection_only_for_a_restore_whose_page_hide_closed_the_socket() {
        assert_eq!(
            PageShowKind::RestoredFromBackForwardCache.get_effect(PageHideClosureKind::SocketClosed),
            PageShowEffectKind::ConnectionLost,
        );
        assert_eq!(
            PageShowKind::RestoredFromBackForwardCache.get_effect(PageHideClosureKind::NoSocket),
            PageShowEffectKind::Unchanged,
        );
        assert_eq!(
            PageShowKind::Loaded.get_effect(PageHideClosureKind::SocketClosed),
            PageShowEffectKind::Unchanged,
        );
        assert_eq!(
            PageShowKind::Loaded.get_effect(PageHideClosureKind::NoSocket),
            PageShowEffectKind::Unchanged,
        );
    }
}
