#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    Quit,
    SwitchFocus,
    ToggleMenu,
    ResetLayout,
    DumpHistory,
    ScrollUp,
    ScrollDown,
    ExpandDir,
    CollapseDir,
    Open,
    FileSearch,
    None,
}
