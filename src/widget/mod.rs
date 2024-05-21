use iced::advanced::widget::{ Id, Operation, operation::Outcome };
use std::path::Path;

pub mod dir_entry;

use dir_entry::DirEntry;


pub fn dir_entry<Message, P>(path: P) -> DirEntry<Message>
where
    P: AsRef<Path>,
    Message: Clone,
{
    DirEntry::new(path)
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
