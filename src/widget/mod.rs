pub mod dir_entry;
pub mod fuzzy_input;
pub mod tag_entry;
pub mod context_menu;
pub mod notification_card;


/// Create a [`iced::widget::Text`] widget with the given [`Bootstrap`] icon
/// ```
/// icon!(`Bootstrap`)          // Just the icon
/// icon!(`Bootstrap`, light)   // Light colored icon
/// icon!(`Bootstrap`, `Color`) // Colored icon
/// ```
#[macro_export]
macro_rules! icon {
    ($i:expr) => {
        iced::widget::Text::new(
            iced_aw::core::icons::bootstrap::icon_to_string($i)
        ).font(iced_aw::core::icons::BOOTSTRAP_FONT)
    };
    
    ($i:expr, light) => {
        iced::widget::Text::new(
            iced_aw::core::icons::bootstrap::icon_to_string($i)
        )
        .font(iced_aw::core::icons::BOOTSTRAP_FONT)
        .style($crate::app::theme::LIGHT_TEXT_COLOR)
    };

    ($i:expr, $col:expr) => {
        iced::widget::Text::new(
            iced_aw::core::icons::bootstrap::icon_to_string($i)
        )
        .font(iced_aw::core::icons::BOOTSTRAP_FONT)
        .style($col)
    };
}


/// ```
/// simple_button!(icon = `Bootstrap`)    // Button with just the icon
/// simple_button!(inner)                 // Button with whatever inside
/// simple_button!(inner, text)           // Button with whatever inside and some text
/// ```
#[macro_export]
macro_rules! simple_button {
    (icon = $icon:expr) => {
        iced::widget::button(icon!($icon, light))
            .style($crate::app::theme::Simple)
    };

    ($inner:expr) => {
        iced::widget::button($inner)
            .style($crate::app::theme::Simple)
    };

    ($inner:expr, $text:expr) => {
        iced::widget::button(iced::widget::row![
            $inner,
            iced::widget::text($text) .style(iced::Color::new(0.8, 0.84, 0.95, 1.0)),
        ])
        .style($crate::app::theme::Simple)
    };
}






