#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteDefault {
    Never,
    IfMissing,
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodePolicy {
    UseDefault,
    RemoveAndUseDefault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovePolicy {
    UseDefault,
    Ignore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistMode {
    Immediate,
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStrategy {
    None,
    CrossContext,
}
