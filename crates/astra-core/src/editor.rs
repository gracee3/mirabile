use serde::{Deserialize, Serialize};

use crate::Revision;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EditorState<T> {
    Clean {
        revision: Revision,
    },
    Dirty {
        base_revision: Revision,
        draft: T,
    },
    Saving {
        base_revision: Revision,
        draft: T,
    },
    Conflict {
        local: T,
        base_revision: Revision,
        remote_revision: Revision,
    },
}

impl<T: Clone> EditorState<T> {
    pub const fn clean(revision: Revision) -> Self {
        Self::Clean { revision }
    }

    pub fn edit(self, canonical: &T, edit: impl FnOnce(&mut T)) -> Self {
        match self {
            Self::Clean { revision } => {
                let mut draft = canonical.clone();
                edit(&mut draft);
                Self::Dirty {
                    base_revision: revision,
                    draft,
                }
            }
            Self::Dirty {
                base_revision,
                mut draft,
            }
            | Self::Saving {
                base_revision,
                mut draft,
            } => {
                edit(&mut draft);
                Self::Dirty {
                    base_revision,
                    draft,
                }
            }
            Self::Conflict {
                mut local,
                base_revision,
                remote_revision,
            } => {
                edit(&mut local);
                Self::Conflict {
                    local,
                    base_revision,
                    remote_revision,
                }
            }
        }
    }

    pub fn effective<'a>(&'a self, canonical: &'a T) -> &'a T {
        match self {
            Self::Clean { .. } => canonical,
            Self::Dirty { draft, .. } | Self::Saving { draft, .. } => draft,
            Self::Conflict { local, .. } => local,
        }
    }

    pub fn begin_save(self) -> Self {
        match self {
            Self::Dirty {
                base_revision,
                draft,
            } => Self::Saving {
                base_revision,
                draft,
            },
            other => other,
        }
    }

    pub fn saved(self, revision: Revision) -> Self {
        Self::Clean { revision }
    }

    pub fn conflict(self, remote_revision: Revision) -> Self {
        match self {
            Self::Dirty {
                base_revision,
                draft,
            }
            | Self::Saving {
                base_revision,
                draft,
            } => Self::Conflict {
                local: draft,
                base_revision,
                remote_revision,
            },
            other => other,
        }
    }

    pub const fn base_revision(&self) -> Revision {
        match self {
            Self::Clean { revision } => *revision,
            Self::Dirty { base_revision, .. }
            | Self::Saving { base_revision, .. }
            | Self::Conflict { base_revision, .. } => *base_revision,
        }
    }

    pub const fn is_dirty(&self) -> bool {
        !matches!(self, Self::Clean { .. })
    }

    pub const fn is_saving(&self) -> bool {
        matches!(self, Self::Saving { .. })
    }

    pub fn cancel(self, canonical_revision: Revision) -> Self {
        Self::Clean {
            revision: canonical_revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Angle, AspectClass, AspectDefinition, AspectId, AspectSet, OrbPolicy};

    use super::*;

    fn aspect_set() -> AspectSet {
        AspectSet {
            aspects: vec![AspectDefinition {
                id: AspectId::new("conjunction").expect("valid ID"),
                name: "Conjunction".into(),
                angle: Angle::from_degrees(0.0).expect("valid angle"),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: Angle::from_degrees(8.0).expect("valid angle"),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            }],
        }
    }

    #[test]
    fn draft_isolated_until_save_and_cancel_restores_canonical() {
        let canonical = aspect_set();
        let state = EditorState::clean(Revision::INITIAL).edit(&canonical, |draft| {
            draft.aspects[0].orbs.maximum = Angle::from_degrees(7.0).expect("valid angle");
        });

        assert_eq!(canonical.aspects[0].orbs.maximum.degrees(), 8.0);
        assert_eq!(
            state.effective(&canonical).aspects[0]
                .orbs
                .maximum
                .degrees(),
            7.0
        );

        let canceled = state.cancel(Revision::INITIAL);
        assert_eq!(
            canceled.effective(&canonical).aspects[0]
                .orbs
                .maximum
                .degrees(),
            8.0
        );
        assert!(!canceled.is_dirty());
    }
}
