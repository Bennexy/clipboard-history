# Clipboard History manager for linux X11 Wayland
a fully rust based unix only application featuring a high speed 
and light weight daemon and a UI weitten in Egui.

# Instalation
> disclaimer! this is a early development version
> the documentation of the current state and features is not up to date.
>

currently no .deb or binary is available. compilation from source is required.
rust and rustc is required.

run the Daemon
'caro run --project clipd --release'
run the UI
'cargo run --project clipui --release'

## Clipd Daemon 
listens for changes on the clipboard

stores the text based entries in a sqlite db which is stored innmemory or in tmp disk.
the store location is user configurable.

## ClipUi Egui based User interface
displays the historic clipboard text entries in a descending order includes a full text search

### project Goals
wirte a lightweight Highperformance, Minimalistic, 
Clipboard History Manager, which mimiks the Windows Clipboard, 
History, Communication between UI and Daemon 
via Unix Socket, Fulltextsearch, 
Automatic Population the Clipboard on select 

