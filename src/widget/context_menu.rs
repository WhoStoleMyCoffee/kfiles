//! A context menu for showing actions on click
//!

use std::rc::Rc;

use iced::{
    advanced::{
        layout::{Limits, Node},
        overlay, renderer,
        widget::{tree, Operation, Tree},
        Clipboard, Layout, Shell, Widget,
    }, event::{self, Status}, keyboard, mouse::{self, Button as MouseButton, Cursor}, touch, window, Border, Element, Event, Length, Point, Rectangle, Shadow, Size, Vector
};
use iced::{Background, Color, Theme};




pub struct ContextMenu<'a, Overlay, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Overlay: Fn() -> Element<'a, Message, Theme, Renderer>,
    Message: Clone,
    Renderer: renderer::Renderer,
    Theme: StyleSheet,
{
    /// The underlying element.
    underlay: Element<'a, Message, Theme, Renderer>,
    /// The content of [`ContextMenuOverlay`].
    overlay: Overlay,
    /// The style of the [`ContextMenu`].
    style: <Theme as StyleSheet>::Style,
    /// Event to listen to to open
    open_event: iced::Event,
    offset: Vector,
}

impl<'a, Overlay, Message, Theme, Renderer> ContextMenu<'a, Overlay, Message, Theme, Renderer>
where
    Overlay: Fn() -> Element<'a, Message, Theme, Renderer>,
    Message: Clone,
    Renderer: renderer::Renderer,
    Theme: StyleSheet,
{
    /// Creates a new [`ContextMenu`]
    ///
    /// `underlay`: The underlying element.
    ///
    /// `overlay`: The content of [`ContextMenuOverlay`] which will be displayed when `underlay` is clicked.
    pub fn new<U>(underlay: U, overlay: Overlay) -> Self
    where
        U: Into<Element<'a, Message, Theme, Renderer>>,
    {
        ContextMenu {
            underlay: underlay.into(),
            overlay,
            style: <Theme as StyleSheet>::Style::default(),
            open_event: iced::Event::Mouse(mouse::Event::ButtonPressed(MouseButton::Right)),
            offset: Vector::ZERO,
        }
    }

    /// Sets the style of the [`ContextMenu`].
    pub fn style(mut self, style: <Theme as StyleSheet>::Style) -> Self {
        self.style = style;
        self
    }

    pub fn custom_event(mut self, event: iced::Event) -> Self {
        self.open_event = event;
        self
    }

    /// Make this [`ContextMenu`] open by left clicking instead of right clicking
    /// This is a shorthand for [`ContextMenu::custom_event`]
    pub fn left_click_activated(mut self) -> Self {
        self.open_event = iced::Event::Mouse(mouse::Event::ButtonPressed(MouseButton::Left));
        self
    }

    /// Make this [`ContextMenu`] open by releasing a left click instead of right clicking
    /// This is a shorthand for [`ContextMenu::custom_event`]
    pub fn left_click_release_activated(mut self) -> Self {
        self.open_event = iced::Event::Mouse(mouse::Event::ButtonReleased(MouseButton::Left));
        self
    }

    pub fn offset(mut self, offset: impl Into<Vector>) -> Self {
        self.offset = offset.into();
        self
    }

}


impl<'a, Content, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ContextMenu<'a, Content, Message, Theme, Renderer>
where
    Content: 'a + Fn() -> Element<'a, Message, Theme, Renderer>,
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: StyleSheet,
{
    fn size(&self) -> iced::Size<Length> {
        self.underlay.as_widget().size()
    }

    fn layout(&self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.underlay
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.underlay), Tree::new(&(self.overlay)())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.underlay, &(self.overlay)()]);
    }

    fn operate<'b>(
        &'b self,
        state: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<Message>,
    ) {
        let s: &mut State = state.state.downcast_mut();

        if s.show {
            let content = (self.overlay)();
            content.as_widget().diff(&mut state.children[1]);

            content
                .as_widget()
                .operate(&mut state.children[1], layout, renderer, operation);
        } else {
            self.underlay
                .as_widget()
                .operate(&mut state.children[0], layout, renderer, operation);
        }
    }

    fn on_event(
        &mut self,
        state: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        if event == self.open_event {
            let bounds = layout.bounds();

            if cursor.is_over(bounds) {
                let s: &mut State = state.state.downcast_mut();
                s.cursor_position = cursor.position().unwrap_or_default();
                s.show = !s.show;
                return event::Status::Captured;
            }
        }

        self.underlay.as_widget_mut().on_event(
            &mut state.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.underlay.as_widget().mouse_interaction(
            &state.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let s: &mut State = state.state.downcast_mut();

        if !s.show {
            return self.underlay.as_widget_mut().overlay(
                &mut state.children[0],
                layout,
                renderer,
                translation,
            );
        }

        let position: Point = s.cursor_position + self.offset;
        let content = (self.overlay)();
        content.as_widget().diff(&mut state.children[1]);
        Some(
            ContextMenuOverlay::new(
                position + translation,
                &mut state.children[1],
                content,
                self.style.clone(),
                s,
            )
            .overlay(),
        )
    }
}

impl<'a, Content, Message, Theme, Renderer> From<ContextMenu<'a, Content, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Content: 'a + Fn() -> Self,
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: 'a + StyleSheet,
{
    fn from(modal: ContextMenu<'a, Content, Message, Theme, Renderer>) -> Self {
        Element::new(modal)
    }
}

/// The state of the ``context_menu``.
#[derive(Debug, Default)]
pub(crate) struct State {
    /// The visibility of the [`ContextMenu`] overlay.
    pub show: bool,
    /// Use for showing the overlay where the click was made.
    pub cursor_position: Point,
}

impl State {
    /// Creates a new [`State`] containing the given state data.
    pub const fn new() -> Self {
        Self {
            show: false,
            cursor_position: Point::ORIGIN,
        }
    }
}




/// The overlay of the [`ContextMenu`](crate::native::ContextMenu).
pub struct ContextMenuOverlay<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: StyleSheet,
{
    // The position of the element
    position: Point,
    /// The state of the [`ContextMenuOverlay`].
    tree: &'a mut Tree,
    /// The content of the [`ContextMenuOverlay`].
    content: Element<'a, Message, Theme, Renderer>,
    /// The style of the [`ContextMenuOverlay`].
    style: <Theme as StyleSheet>::Style,
    /// The state shared between [`ContextMenu`](crate::native::ContextMenu) and [`ContextMenuOverlay`].
    state: &'a mut State,
}

impl<'a, Message, Theme, Renderer> ContextMenuOverlay<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
    Theme: 'a + StyleSheet,
{
    /// Creates a new [`ContextMenuOverlay`].
    pub(crate) fn new<C>(
        position: Point,
        tree: &'a mut Tree,
        content: C,
        style: <Theme as StyleSheet>::Style,
        state: &'a mut State,
    ) -> Self
    where
        C: Into<Element<'a, Message, Theme, Renderer>>,
    {
        ContextMenuOverlay {
            position,
            tree,
            content: content.into(),
            style,
            state,
        }
    }

    /// Turn this [`ContextMenuOverlay`] into an overlay [`Element`](overlay::Element).
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }
}

impl<'a, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for ContextMenuOverlay<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: StyleSheet,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let limits = Limits::new(Size::ZERO, bounds);
        let max_size = limits.max();

        let mut content = self
            .content
            .as_widget()
            .layout(self.tree, renderer, &limits);

        // Try to stay inside borders
        let mut position = self.position;
        if position.x + content.size().width > bounds.width {
            position.x = f32::max(0.0, position.x - content.size().width);
        }
        if position.y + content.size().height > bounds.height {
            position.y = f32::max(0.0, position.y - content.size().height);
        }

        content.move_to_mut(position);

        Node::with_children(max_size, vec![content])
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        let bounds = layout.bounds();

        let style_sheet = theme.active(&self.style);

        // Background
        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: (0.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: Shadow::default(),
                },
                style_sheet.background,
            );
        }

        let content_layout = layout
            .children()
            .next()
            .expect("Native: Layout should have a content layout.");

        // Modal
        self.content.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            content_layout,
            cursor,
            &bounds,
        );
    }

    fn on_event(
        &mut self,
        event: Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<Message>,
    ) -> Status {
        let layout_children = layout
            .children()
            .next()
            .expect("Native: Layout should have a content layout.");

        let mut forward_event_to_children = true;

        let status = match &event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                if *key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    self.state.show = false;
                    forward_event_to_children = false;
                    Status::Captured
                } else {
                    Status::Ignored
                }
            }

            Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left | mouse::Button::Right,
            ))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if !cursor.is_over(layout_children.bounds()) {
                    self.state.show = false;
                    forward_event_to_children = false;
                }
                Status::Captured
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                // close when released because because button send message on release
                self.state.show = false;
                Status::Captured
            }

            Event::Window(_id, window::Event::Resized { .. }) => {
                self.state.show = false;
                forward_event_to_children = false;
                Status::Captured
            }

            _ => Status::Ignored,
        };

        let child_status = if forward_event_to_children {
            self.content.as_widget_mut().on_event(
                self.tree,
                event,
                layout_children,
                cursor,
                renderer,
                clipboard,
                shell,
                &layout.bounds(),
            )
        } else {
            Status::Ignored
        };

        match child_status {
            Status::Ignored => status,
            Status::Captured => Status::Captured,
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            self.tree,
            layout
                .children()
                .next()
                .expect("Native: Layout should have a content layout."),
            cursor,
            viewport,
            renderer,
        )
    }
}




/// The appearance of a [`ContextMenu`]
#[derive(Clone, Copy, Debug)]
pub struct Appearance {
    /// The background of the [`ContextMenu`](crate::native::ContextMenu).
    ///
    /// This is used to color the backdrop of the modal.
    pub background: Background,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            background: Background::Color([0.87, 0.87, 0.87, 0.30].into()),
        }
    }
}

/// The appearance of a [`ContextMenu`](crate::native::ContextMenu).
pub trait StyleSheet {
    ///Style for the trait to use.
    type Style: Default + Clone;
    /// The normal appearance of a [`ContextMenu`](crate::native::ContextMenu).
    fn active(&self, style: &Self::Style) -> Appearance;
}

/// The default appearance of a [`ContextMenu`](crate::native::ContextMenu).
#[derive(Clone, Default)]
pub enum ContextMenuStyle {
    #[default]
    Default,
    Custom(Rc<dyn StyleSheet<Style = Theme>>),
}

impl ContextMenuStyle {
    /// Creates a custom [`ContextMenuStyle`] style variant.
    pub fn custom(style_sheet: impl StyleSheet<Style = Theme> + 'static) -> Self {
        Self::Custom(Rc::new(style_sheet))
    }
}

impl StyleSheet for Theme {
    type Style = ContextMenuStyle;

    fn active(&self, _style: &Self::Style) -> Appearance {
        let palette = self.extended_palette();

        Appearance {
            background: Color {
                a: 0f32,
                ..palette.background.base.color
            }
            .into(),
        }
    }
}


