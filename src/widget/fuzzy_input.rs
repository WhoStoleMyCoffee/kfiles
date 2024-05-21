use iced::advanced::widget::Tree;
use iced::advanced::{Layout, Widget};
use iced::{mouse, widget, Element, Length, Rectangle, Size};
use iced::{
    advanced,
    overlay::menu,
};
use iced::{
    event, Event,
    keyboard::{ self, key, Key }
};
use iced::widget::{
    container,
    scrollable,
    text_input, TextInput,
};


pub struct FuzzyInput<
    'a,
    T,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
>
where
    T: ToString + PartialEq + Clone,
    Message: Clone,
    Theme: text_input::StyleSheet
        + menu::StyleSheet
        + scrollable::StyleSheet
        + container::StyleSheet,
    Renderer: advanced::text::Renderer,
{
    text_input: TextInput<'a, Message, Theme, Renderer>,
    options: &'a [T],
    on_selected: Box<dyn Fn(T) -> Message + 'a>,
    on_hovered: Option<Box<dyn Fn(T) -> Message + 'a>>,
    query: String,
}


impl<'a, T, Message, Theme, Renderer> FuzzyInput<'a, T, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone,
    Message: Clone,
    Theme: text_input::StyleSheet
        + menu::StyleSheet
        + scrollable::StyleSheet
        + container::StyleSheet,
    Renderer: advanced::text::Renderer,
{
    pub fn new<F>(
        placeholder_text: &str,
        text: &str,
        options: &'a [T],
        on_selected: F,
    ) -> Self
    where
        F: 'a + Fn(T) -> Message,
    {
        FuzzyInput {
            text_input: text_input(placeholder_text, text),
            options,
            on_selected: Box::new(on_selected),
            on_hovered: None,
            query: text.to_string(),
        }
    }

    pub fn text_input<F>(mut self, f: F) -> Self
    where
        F: FnOnce(TextInput<'a, Message, Theme, Renderer>) -> TextInput<'a, Message, Theme, Renderer> + 'a,
    {
        self.text_input = f(self.text_input);
        self
    }

    pub fn on_hovered<F>(mut self, f: F) -> Self
    where
        F: Fn(T) -> Message + 'a
    {
        self.on_hovered = Some(Box::new(f));
        self
    }
}



impl<'a, T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FuzzyInput<'a, T, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone + 'static,
    Message: 'a + Clone,
    Theme: text_input::StyleSheet
        + menu::StyleSheet
        + scrollable::StyleSheet
        + container::StyleSheet,
    Renderer: 'a + advanced::text::Renderer,
{
    fn size(&self) -> Size<Length> {
        (&self.text_input as &dyn Widget<_, _, _>).size()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new( &self.text_input as &dyn Widget<_, _, _,> )]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[ &self.text_input as &dyn Widget<_, _, _,> ]);
    }

    fn state(&self) -> advanced::widget::tree::State {
        advanced::widget::tree::State::new(FuzzyState::<T>::default())
    }

    fn tag(&self) -> advanced::widget::tree::Tag {
        advanced::widget::tree::Tag::of::< FuzzyState<T> >()
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &advanced::layout::Limits,
    ) -> advanced::layout::Node {
        (&self.text_input as &dyn Widget<_, _, _>).layout(
            &mut tree.children[0],
            renderer,
            limits
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &advanced::renderer::Style,
        layout: advanced::Layout<'_>,
        cursor: advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        (&self.text_input as &dyn Widget<_, _, _>).draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: advanced::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn advanced::widget::Operation<Message>,
    ) {
        (&self.text_input as &dyn Widget<_, _, _>).operate(
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        )
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: iced::Event,
        layout: advanced::Layout<'_>,
        cursor: advanced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn advanced::Clipboard,
        shell: &mut advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) -> advanced::graphics::core::event::Status {
        let state: &mut FuzzyState<T> = tree.state.downcast_mut();

        // man thats a disgusting amount of indentation
        if state.is_expanded {
            match &event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    match key {
                        Key::Named(key::Named::ArrowUp) if modifiers.is_empty() => {
                            let options: &[T] = match &state.filtered_options {
                                Some(v) => v,
                                None => self.options,
                            };
                                

                            let index = state.hovered_option.map_or(
                                options.len() - 1,
                                |i| if i == 0 { options.len() - 1 } else { i - 1 }
                            );
                            state.hovered_option = Some(index);

                            if let Some(on_hovered) = &self.on_hovered {
                                shell.publish( (on_hovered)( options[index].clone() ) );
                            }
                            
                            return event::Status::Captured;
                        }

                        Key::Named(key::Named::ArrowDown) if modifiers.is_empty() => {
                            let options: &[T] = match &state.filtered_options {
                                Some(v) => v,
                                None => self.options,
                            };

                            let index = state.hovered_option.map_or(0, |i| (i + 1) % options.len());
                            state.hovered_option = Some(index);

                            if let Some(on_hovered) = &self.on_hovered {
                                shell.publish( (on_hovered)( options[index].clone() ) );
                            }

                            return event::Status::Captured;
                        }

                        Key::Named(key::Named::Enter) if modifiers.is_empty() => {
                            if let Some(index) = state.hovered_option {
                                let options = match &state.filtered_options {
                                    Some(options) => options,
                                    None => self.options,
                                };

                                shell.publish( (self.on_selected)( options[index].clone() ) );
                                return event::Status::Captured;
                            }
                        }

                        _ => {}
                    }
                }

                _ => {}
            }

            // Query changed
            if self.query != state.query {
                state.query = self.query.clone();

                state.filtered_options = if self.query.is_empty() {
                    None
                } else {
                    Some(self.options.iter()
                         // TODO fuzzy match
                        .filter(|opt| opt.to_string().to_lowercase() .contains( &self.query.to_lowercase() ) )
                        .cloned()
                        .collect()
                    )
                };

                state.hovered_option = Some(0);
            }
        }

        let textinput_tree = &mut tree.children[0];
        let res = (&mut self.text_input as &mut dyn Widget<_, _, _>).on_event(
            textinput_tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let text_state: &widget::text_input::State<Renderer::Paragraph> = textinput_tree.state.downcast_ref();
        let should_be_expanded = text_state.is_focused();
        if state.is_expanded != should_be_expanded {
            state.is_expanded = should_be_expanded;
            state.hovered_option = Some(0);
        }

        res
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        (&self.text_input as &dyn Widget<_, _, _>).mouse_interaction(
            &state.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        translation: iced::Vector,
    ) -> Option<advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        let state: &mut FuzzyState<T> = tree.state.downcast_mut();

        if !state.is_expanded {
            return None;
        }
        
        let options = match &state.filtered_options {
            Some(options) => options,
            None => self.options,
        };
        
        let bounds = layout.bounds();
        let menu = menu::Menu::new(
            &mut state.menu,
            options,
            &mut state.hovered_option,
            &self.on_selected,
            self.on_hovered.as_deref(),
        )
        .width(bounds.width);

        Some(menu.overlay( layout.position() + translation, bounds.height ))
    }
}


impl<'a, T, Message, Theme, Renderer>
From<FuzzyInput<'a, T, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone + 'static,
    Message: 'a + Clone,
    Theme: 'a + text_input::StyleSheet
        + menu::StyleSheet
        + scrollable::StyleSheet
        + container::StyleSheet,
    Renderer: 'a + advanced::text::Renderer,
{
    fn from(value: FuzzyInput<'a, T, Message, Theme, Renderer>) -> Self {
        Element::new(value)
    }
}


#[derive(Debug)]
struct FuzzyState<T>
where
    T: ToString + PartialEq + Clone + 'static
{
    menu: menu::State,
    hovered_option: Option<usize>,
    is_expanded: bool,
    query: String,
    filtered_options: Option<Vec<T>>,
}

impl<T> Default for FuzzyState<T>
where
    T: ToString + PartialEq + Clone
{
    fn default() -> Self {
        Self {
            menu: menu::State::default(),
            hovered_option: None,
            is_expanded: false,
            query: String::new(),
            filtered_options: None,
        }
    }
}
