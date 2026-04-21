### Installation
First install **`nightly`** for Rust, then run **`rustup default nightly`**.
Second download the source code.  
Third read the Compile section.   

### Compile
For compiling use **`cargo build`** (So if you attempt to run this without my fork on mlua in the same directory as the LuantiBot directory, it will fail to compile correctly. I strongly recommend that you make a new folder name it whatever, put my fork of mlua in there and the LuantiBot code in there. (The entire folder structure should look something roughly like this
`**yourFolderNameYouChose/mlua**`
and `**yourFolderNameYouChose/luantibot**`, then change your directory into **luantibot** and run **`cargo build`**)

### Running the bot
For running the bot do **`cargo run --release --bin lua_bot serverIP:port name password path/to/bot/lua/file`**.
It is not very simple yet, that is something i have to work on.

### Credits
I give a lot of credit to minetest-rust ( https://github.com/minetest-rust/ ). They have some network related repos i forked and patched to make this project even possible.

### License
This project in under the GNU General Public License v3.0. For more info see LICENSE.

### I am not accepting commits to this project.
I work on solo projects only, if you want something changed makes a Issue
and I will get that changed if it makes sense(example of something I would not add:
***`Add a fly feature but only if falling from exactly 65 blocks with exactly 96 ping and the bot's velocity is 250`***)
