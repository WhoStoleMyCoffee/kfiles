use iced::advanced::widget::{ operation::Outcome, Id, Operation };

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
        .style(iced::Color::new(0.8, 0.84, 0.95, 1.0))
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
            .style( iced::theme::Button::custom($crate::app::theme::button::Simple) )
    };

    ($inner:expr) => {
        iced::widget::button($inner)
            .style( iced::theme::Button::custom($crate::app::theme::button::Simple) )
    };

    ($inner:expr, $text:expr) => {
        iced::widget::button(iced::widget::row![
            $inner,
            iced::widget::text($text) .style(iced::Color::new(0.8, 0.84, 0.95, 1.0)),
        ])
            .style( iced::theme::Button::custom($crate::app::theme::button::Simple) )
    };
}







pub fn is_focused(id: Id) -> impl Operation<bool> {
    struct IsFocused {
        widget_id: Id,
        result: Option<bool>,
    }

    impl Operation<bool>  for IsFocused {
        fn focusable(&mut self, state: &mut dyn iced::advanced::widget::operation::Focusable, id: Option<&Id>) {
            if let Some(id) = id {
                if *id == self.widget_id {
                    self.result = Some(state.is_focused());
                }
            }
        }

        fn container(
            &mut self,
            _id: Option<&Id>,
            _bounds: iced::Rectangle,
            operate_on_children: &mut dyn FnMut(&mut dyn Operation<bool>),
        ) {
            operate_on_children(self);
        }

        fn finish(&self) -> iced::advanced::widget::operation::Outcome<bool> {
            match self.result {
                Some(is_focused) => Outcome::Some(is_focused),
                None => Outcome::None,
            }
        }
    }

    IsFocused {
        widget_id: id,
        result: None,
    }
}


