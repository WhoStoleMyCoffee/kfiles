use std::iter;
use std::ops::Range;
use std::path::PathBuf;
use std::{marker::PhantomData, path::Path};

use iced::{Element, Length, Size};
use iced::widget::{component, horizontal_space, Component};
use iced_aw::widgets::Wrap;

use super::dir_entry::DirEntry;

const ITEM_SIZE: (f32, f32) = (80.0, 120.0);
const ITEM_SPACING: (f32, f32) = (8.0, 8.0);
const TOTAL_ITEM_SIZE: (f32, f32) = (ITEM_SIZE.0 + ITEM_SPACING.0, ITEM_SIZE.1 + ITEM_SPACING.1);



#[derive(Debug, Clone)]
pub enum Event {
    EntryHovered(usize),
    EntrySelected(usize),
    EntryRightClickSelected(usize),
    EntryActivated(usize),
}


#[derive(Debug, Default)]
pub struct State {
    // todo what to do with this shit :D
    hovered_index: Option<usize>
}


pub struct FileList<'a, Message, Item>
where 
    Message: Clone,
    Item: AsRef<Path>,
{
    items: &'a [Item],
    selected_item: Option<&'a Path>,
    width: Length,
    height: Length,
    // This widget's bounds for culling
    cull_size: Option<Size>,
    scroll: f32,
    // id: Id,
    on_item_hover: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    on_item_select: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    on_item_activate: Option<Box<dyn Fn(PathBuf) -> Message + 'a>>,
    on_item_right_click_select: Option<Box<dyn Fn(usize) -> Message + 'a>>,
}


impl<'a, Message, Item> FileList<'a, Message, Item>
where 
    Message: Clone,
    Item: AsRef<Path>,
{
    pub fn new(items: &'a [Item]) -> Self {
        FileList {
            items,
            selected_item: None,
            width: Length::Fill,
            height: Length::Shrink,
            cull_size: None,
            scroll: 0.0,
            on_item_hover: None,
            on_item_select: None,
            on_item_right_click_select: None,
            on_item_activate: None,
        }
    }

    /// The default width is [`Length::Fill`]
    #[inline]
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// The default height is [`Length::Shrink`]
    #[inline]
    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    #[inline]
    pub fn cull<S: Into<Size>>(mut self, size: Option<S>, scroll: f32) -> Self {
        self.cull_size = size.map(|s| s.into());
        self.scroll = scroll;
        self
    }

    pub fn on_item_hovered<F>(mut self, f: F) -> Self
    where F: Fn(usize) -> Message + 'a
    {
        self.on_item_hover = Some(Box::new(f));
        self
    }

    pub fn on_item_selected<F>(mut self, f: F) -> Self
    where F: Fn(usize) -> Message + 'a
    {
        self.on_item_select = Some(Box::new(f));
        self
    }

    pub fn on_item_right_click_selected<F>(mut self, f: F) -> Self
    where F: Fn(usize) -> Message + 'a
    {
        self.on_item_right_click_select = Some(Box::new(f));
        self
    }

    pub fn on_item_activated<F>(mut self, f: F) -> Self
    where F: Fn(PathBuf) -> Message + 'a
    {
        self.on_item_activate = Some(Box::new(f));
        self
    }

    pub fn with_selected_maybe(mut self, item: Option<&'a Path>) -> Self {
        self.selected_item = item;
        self
    }

    pub fn with_selected(mut self, item: &'a Path) -> Self {
        unimplemented!()
    }

    fn view_dir_entry(&self, item: &Item, index: usize) -> DirEntry<Event> {
        DirEntry::new(item)
            .is_selected( self.selected_item.as_ref().is_some_and(|p| *p == item.as_ref()) )
            .width(ITEM_SIZE.0)
            .height(ITEM_SIZE.1)
            .on_hover(Event::EntryHovered(index))
            .on_activate(Event::EntryActivated(index))
            .on_click(Event::EntrySelected(index))
            .on_right_click(Event::EntryRightClickSelected(index))
    }

    fn view_unculled(&self) -> Element<'_, Event, iced::Theme, iced::Renderer> {
        Wrap::with_elements(
            self.items.iter().enumerate() .map(|(i, item)|
                self.view_dir_entry(item, i).into()
            )
            .collect()
        )
        .spacing(ITEM_SPACING.0)
        .line_spacing(ITEM_SPACING.1).width_items(self.width)
            .height_items(self.height)
            .into()
    }

    fn view_culled(&self, cull_size: &Size) -> Element<'_, Event, iced::Theme, iced::Renderer> {
        let cols = (cull_size.width / TOTAL_ITEM_SIZE.0) as usize;
        let skipped_rows_count: usize = (self.scroll / TOTAL_ITEM_SIZE.1) as usize;
        let skipped_count = (skipped_rows_count * cols).min(self.items.len());

        // Empty space before visible entries
        let before = (skipped_rows_count > 0).then(|| horizontal_space()
            .height(TOTAL_ITEM_SIZE.1 * skipped_rows_count as f32)
            .into()
        );

        let visible_rows_count: usize = (cull_size.height / TOTAL_ITEM_SIZE.1) as usize + 2;
        // Iterator over visible entries
        let it = self.items[skipped_count..].iter().enumerate()
            .map(|(i, item)|
                self.view_dir_entry(item, i + skipped_count)
                    .into()
            )
            .take(visible_rows_count * cols);

        let after_count = (self.items.len().div_ceil(cols))
            .checked_sub(skipped_rows_count + visible_rows_count)
            .unwrap_or_default();
        // Empty space after visible entries
        let after = (after_count > 0).then(|| horizontal_space()
            .height(TOTAL_ITEM_SIZE.1 * after_count as f32)
            .into()
        );

        Wrap::with_elements(
            before.into_iter()
                .chain(it)
                .chain(after.into_iter())
                .collect() 
        )
        .spacing(ITEM_SPACING.0)
        .line_spacing(ITEM_SPACING.1).width_items(self.width)
            .height_items(self.height)
            .into()
    }

}


impl<'a, Message, Item> Component<Message> for FileList<'a, Message, Item>
where
    Message: Clone,
    Item: AsRef<Path>,
{
    type Event = Event;
    type State = State;

    fn update(
        &mut self,
        state: &mut Self::State,
        event: Self::Event,
    ) -> Option<Message> {
        match event {
            Event::EntryHovered(index) => {
                state.hovered_index = Some(index);
                return self.on_item_hover.as_ref().map(|f| f(index));
            }

            Event::EntryActivated(index) => {
                if let Some(item) = self.items.get(index) {
                    return self.on_item_activate.as_ref()
                        .map(|f| f( item.as_ref().to_path_buf() ));
                }
            }

            Event::EntrySelected(index) => {
                return self.on_item_select.as_ref().map(|f| f(index));
            }

            Event::EntryRightClickSelected(index) => {
                return self.on_item_right_click_select.as_ref().map(|f| f(index))
            }
        }

        None
    }

    fn view(
        &self,
        _state: &Self::State,
    ) -> iced::advanced::graphics::core::Element<'_, Self::Event, iced::Theme, iced::Renderer> {
        match &self.cull_size {
            Some(s) => self.view_culled(&s),
            None => self.view_unculled(),
        }
    }

    fn size_hint(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }
}


impl<'a, Message, Item> From<FileList<'a, Message, Item>> for Element<'a, Message>
where 
    Message: 'a + Clone,
    Item: 'a + AsRef<Path>,
{
    fn from(value: FileList<'a, Message, Item>) -> Self {
        component(value)
    }
}





/// Get the range of items which are visible in a [`FileList`]
/// come to think of it, there was probably a better way to do all this culling thing
pub fn get_visible_items_range(width: f32, height: f32, scroll: f32) -> Range<usize> {
    let items_per_row: usize = get_items_per_row(width);
    //          (        Which row do we start at?       ) * items per row
    let start = (scroll / TOTAL_ITEM_SIZE.1) as usize * items_per_row;
    let end = start
    //  + (    How many rows does the view span?    ) * items per row
        + ((height / TOTAL_ITEM_SIZE.1) as usize + 2) * items_per_row;

    start..end
}

#[inline]
fn get_items_per_row(width: f32) -> usize {
    (width / TOTAL_ITEM_SIZE.0) as usize
}
