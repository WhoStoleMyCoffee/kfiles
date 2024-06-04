use iced::{advanced::widget::{ operation::Outcome, Id, Operation }, widget::{self, button, Text}, Color, Vector};
use iced_aw::Bootstrap;

pub mod dir_entry;
pub mod fuzzy_input;
pub mod tag_entry;
pub mod context_menu;


/// `icon!(Bootstrap)`
/// `icon!(Bootstrap, light)` for light text
/// `icon!(Bootstrap, iced::Color)`
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


#[macro_export]
macro_rules! icon_button {
    (icon = $icon:expr) => {
        iced::widget::button(icon!($icon, light))
            .style( iced::theme::Button::custom(crate::widget::theme::Simple) )
    };

    ($icon:expr) => {
        iced::widget::button($icon)
            .style( iced::theme::Button::custom(crate::widget::theme::Simple) )
    };

    ($icon:expr, $text:expr) => {
        iced::widget::button(iced::widget::row![
            $icon,
            iced::widget::text($text) .style(iced::Color::new(0.8, 0.84, 0.95, 1.0)),
        ])
            .style( iced::theme::Button::custom(crate::widget::theme::Simple) )
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


pub mod theme {
    use iced::{widget::button, Color, Vector};

    pub struct Simple;

    impl button::StyleSheet for Simple {
        type Style = iced::Theme;

        fn active(&self, _style: &Self::Style) -> button::Appearance {
            button::Appearance {
                background: Some(Color::new(0.17, 0.17, 0.24, 1.0).into()),
                border: iced::Border::with_radius(4.0),
                ..Default::default()
            }
        }

        fn hovered(&self, style: &Self::Style) -> button::Appearance {
            let active = self.active(style);

            button::Appearance {
                background: Some(Color::new(0.20, 0.20, 0.28, 1.0).into()),
                shadow_offset: active.shadow_offset + Vector::new(0.0, 1.0),
                ..active
            }
        }

        /// Produces the pressed [`Appearance`] of a button.
        fn pressed(&self, style: &Self::Style) -> button::Appearance {
            button::Appearance {
                background: Some(Color::new(0.16, 0.15, 0.23, 1.0).into()),
                shadow_offset: Vector::default(),
                ..self.active(style)
            }
        }
    }


}


