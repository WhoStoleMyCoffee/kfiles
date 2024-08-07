use std::cmp::Reverse;

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

use crate::strmatch::{self, StringMatcher};


/// [`TextInput`] extension widget that can fuzzy search from a list of options
/// ```
/// let value = "Some text";
/// let fuzzy_input = FuzzyInput::new(
///     "Query...", 
///     &value,
///     &vec![
///         "Item 1",
///         "Item 2",
///         "Item 3",
///         "Bup time",
///     ],
///     |item| Message::ItemSelected(item),
/// )
/// .text_input(|text_input| {
///     // Configure text input here...
///     text_input.on_input(Message::TextInputChanged)
/// });
/// ```
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
    overlay_style: <Theme as menu::StyleSheet>::Style,
    /// Whether to show options if text input is empty
    hide_on_empty: bool,
}


impl<'a, T, Message, Theme, Renderer> FuzzyInput<'a, T, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone + 'static,
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
            overlay_style: <Theme as menu::StyleSheet>::Style::default(),
            hide_on_empty: false,
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

    pub fn style(
        mut self,
        style: impl Into<<Theme as menu::StyleSheet>::Style>,
    ) -> Self {
        self.overlay_style = style.into();
        self
    }

    /// Sets whether this [`FuzzyInput`]'s dropdown menu should hide if the query is empty
    /// The menu is shown by default, so this is a way to disable it until the user types in
    /// something
    pub fn hide_on_empty(mut self, hide: bool) -> Self {
        self.hide_on_empty = hide;
        self
    }

    /// Filters `self.options` and returns the results
    /// Returns `None` if query is empty
    fn filter(&self, query: &str) -> Option<Vec<T>> {
        if query.is_empty() {
            return None;
        }

        let matcher = strmatch::Sublime::default() .with_query(query);
        let mut matches: Vec<(&T, isize)> = self.options.iter()
            .filter_map(|opt| {
                matcher.score( &opt.to_string() ) .map(|score| (opt, score))
            })
            .collect();
        matches.sort_by_key(|(_opt, score)| Reverse(*score));
        Some(matches.iter()
            .map(|(opt, _score)| (*opt).clone() )
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_event(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        _layout: advanced::Layout<'_>,
        _cursor: advanced::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn advanced::Clipboard,
        shell: &mut advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) -> Option<event::Status>
    {
        let state: &mut FuzzyState<T> = tree.state.downcast_mut();
        if !state.is_expanded {
            return None;
        }

        let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = &event else {
            return None;
        };

        match key {
            Key::Named(key::Named::ArrowUp) if modifiers.is_empty() => {
                self.move_selection(-1, state, Some(shell));
                Some(event::Status::Captured)
            }

            Key::Named(key::Named::ArrowDown) if modifiers.is_empty() => {
                self.move_selection(1, state, Some(shell));
                Some(event::Status::Captured)
            }

            Key::Named(key::Named::Enter) if modifiers.is_empty() => {
                let options = match &state.filtered_options {
                    Some(options) => options,
                    None => self.options,
                };
                
                let Some(selected_option) = options.get(state.hovered_option?) else {
                    state.hovered_option = None;
                    return Some(event::Status::Ignored);
                };

                shell.publish( (self.on_selected)(selected_option.clone()) );
                Some(event::Status::Captured)
            }

            _ => None
        }
    }

    /// Selects the `current + relative`th item, and publishes a message if `shell` is set
    fn move_selection(
        &self,
        relative: isize,
        state: &mut FuzzyState<T>,
        shell: Option< &mut advanced::Shell<'_, Message> >,
    ) {
        let options: &[T] = match &state.filtered_options {
            Some(v) => v,
            None => self.options,
        };

        let index = state.hovered_option
            .and_then(|i| i.checked_add_signed(relative))
            .map(|i| i % options.len())
            .unwrap_or_else(|| if relative >= 0 { 0 } else { options.len() - 1 });

        state.hovered_option = Some(index);

        let Some(shell) = shell else { return; };
        if let Some(on_hovered) = &self.on_hovered {
            shell.publish( (on_hovered)( options[index].clone() ) );
        }
    }
}



impl<'a, T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FuzzyInput<'a, T, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone + 'static,
    Message: 'a + Clone,
    Theme: text_input::StyleSheet
        + menu::StyleSheet<Style = iced::theme::Menu>
        + scrollable::StyleSheet
        + container::StyleSheet,
    // <Theme as iced::overlay::menu::StyleSheet>::Style: From<iced::theme::Menu>,
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
    ) -> event::Status
    {
        if let Some(status) = self.handle_event(tree, &event, layout, cursor, renderer, clipboard, shell, viewport) {
            return status;
        }

        let state: &mut FuzzyState<T> = tree.state.downcast_mut();
        if state.is_expanded {
            // Query changed
            if state.query != self.query {
                state.query.clone_from(&self.query);
                state.filtered_options = self.filter(&self.query);
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
        let should_be_expanded: bool = if self.query.is_empty() && self.hide_on_empty {
            false
        } else {
            text_state.is_focused()
        };

        // Change expansion state if needed
        if state.is_expanded && !should_be_expanded {
            state.is_expanded = false;
            state.hovered_option = None;
            state.filtered_options = None;
        } else if !state.is_expanded && should_be_expanded {
            state.is_expanded = true;
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
    ) -> Option<advanced::overlay::Element<'b, Message, Theme, Renderer>>
    {
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
        .width(bounds.width)
        .style(self.overlay_style.clone());

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
        + menu::StyleSheet<Style = iced::theme::Menu>
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
