use iced::advanced::widget::{ operation::Outcome, Id, Operation };

pub mod dir_entry;
pub mod fuzzy_input;
pub mod tag_entry;
pub mod context_menu;



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
