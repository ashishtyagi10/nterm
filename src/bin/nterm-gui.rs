// nterm GUI - iced-based graphical interface with terminal look and feel

use nterm::gui::app::NtermGui;

fn main() -> iced::Result {
    iced::application(NtermGui::title, NtermGui::update, NtermGui::view)
        .subscription(NtermGui::subscription)
        .theme(NtermGui::theme)
        .window_size((1200.0, 800.0))
        .run_with(NtermGui::new)
}
