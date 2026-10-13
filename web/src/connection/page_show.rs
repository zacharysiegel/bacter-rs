#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageShowEffectKind {
    /// The page left the back/forward cache, and its `pagehide` closed the connection.
    ConnectionLost,
    Unchanged,
}

impl PageShowEffectKind {
    /// `persisted` as `PageTransitionEvent.persisted` gives it.
    pub fn from_persisted(persisted: bool) -> PageShowEffectKind {
        if persisted {
            PageShowEffectKind::ConnectionLost
        } else {
            PageShowEffectKind::Unchanged
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_persisted_loses_the_connection_only_for_a_page_restored_from_the_back_forward_cache() {
        assert_eq!(
            PageShowEffectKind::from_persisted(true),
            PageShowEffectKind::ConnectionLost,
        );
        assert_eq!(PageShowEffectKind::from_persisted(false), PageShowEffectKind::Unchanged);
    }
}
