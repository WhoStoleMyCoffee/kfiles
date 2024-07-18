# KFiles

`kfiles` is an alternative file browser I made because I didn't like Windows File Explorer

## Tags

Unlike other file browsers, `kfiles` makes use of **tags**.

Why is this important?

1.  Categorizing files and folders *by meaning* instead of *by path*.

When you want to go to a friend's house, you don't think *"I'm going to A country, B city, C avenue, #123"*; you think "I'm gonna go see my friend".

When you're looking for an image on your computer, you don't think *"That reminds me of `Pictures/animals/funny_cat.png`"*; you think *"That reminds me of that funny cat picture"*.

Using tags enables you to search through files just like in your brain: *by meaning* instead of *by path*. It removes the "awkward cursor dance" and eye squinting you may do when you know what you're looking for, but can't find it or don't know how to get there.


2. More advanced querying capabilities.

![tags 8](https://github.com/user-attachments/assets/05b551c0-df69-4a6b-aa05-4be6720a2154)

Using tags makes it so you can filter a bunch of directories like a massive Venn diagram.

Looking for all image files related to projects? Just search the intersection between the `#projects` and `#images` tags.

And if you think of it this way, this means you can also have *tags inside of tags*: *"subtags"*.


In KFiles, you can tag single files (e.g. `animals/funny_cat.png` with `#animals` and `#memes`), or entire folders and their contents (e.g. `animals/` with `#animals`).
And of course, one tag can have multiple entries (e.g. `~/Music/` and `~/Documents/MyGame/Music/` could both be tagged with `#music`).


## Querying

![image](https://github.com/user-attachments/assets/47cb4c4d-a3c0-438d-8caa-42bc8ee1ffc4)


On top of tags, `KFiles` offers a sligltly more advanced searching system than your usual *fuzzy-search*.

There are 4 types of search constraints:

- Filter by type with:
  - `--file` or `-f` for files only
  - `--dir` or `-d` for directories (folders) only

- Filter by file extension with `.(ext)` (implies searching for files). E.g.
  - `.rs` will look for only `.rs` files
  - `.txt .toml .json` will look for all files that are either of the 3

- Search for exact matches with `"(query)"`. E.g.
  - `"player"` will filter files and folders that contain *"player"*

- Everything else will be a fuzzy search

### Examples

- `-d kf` wll give you folders (`-d`) that loosely match "kf". E.g.
  - projects/**KF**iles/


- `-f pics "cat"` will give you files (`-f`) that contain the string "cat", and also loosely match "pics". E.g.
  - **Pic**ture**s**/my_**cat**.png

- `.txt "my file` will give you `.txt` files that contain the string "my file".

  Note how in this query, there is no need to fully close the quotes -- `"my file` instead of `"my file"`; these 2 are functionally the same.
  If there is no closing quote, the rest of the query will be matched.
  - Documents/**my file.txt**

Combined with tags:

- `"kfiles" .toml .rs` in `#projects`

  will search through the `#projects` tag, and return all `.toml` and `.rs` files that contain "kfiles" in their path.

- `-f nevgonagiyup "gonna` in `#videos` and `#memes`

  will search through all paths that are tagged with both `#videos` and `#memes`, and return all files (`-f`) that contain "gonna" and loosely match "nevgonagiyup"

  E.g. **Nev**er **gon**n**a** **gi**ve **y**ou **up**.mp4

