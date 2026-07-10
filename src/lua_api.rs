use mlua::{Function, Lua, UserData, UserDataMethods};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;

use crate::{bot::Bot, event::Event};

pub struct LuaBotAPI {
    lua: Lua,
    bot: Arc<Mutex<Bot>>,
    state: Arc<Mutex<crate::state::BotState>>,
}

impl LuaBotAPI {
    pub fn new(bot: Bot) -> Self {
        let state = Arc::new(Mutex::new(bot.state.clone()));
        let bot_ref = Arc::new(Mutex::new(bot));
        let lua = Lua::new();

        Self::setup_lua_globals(&lua, bot_ref.clone(), state.clone());

        Self {
            lua,
            bot: bot_ref,
            state,
        }
    }

    fn setup_lua_globals(lua: &Lua, bot: Arc<Mutex<Bot>>, state: Arc<Mutex<crate::state::BotState>>) {
        let globals = lua.globals();

        let bot_ud = BotLuaWrapper {
            bot: bot.clone(),
            state: state.clone(),
        };

        globals.set("bot", lua.create_userdata(bot_ud).unwrap()).unwrap();
    }

    pub async fn load_script(&self, script_path: &str) -> anyhow::Result<()> {
        let script_content = std::fs::read_to_string(script_path)?;
        self.lua.load(script_content).exec()?;
        Ok(())
    }

    pub async fn run_event_loop(&mut self) -> anyhow::Result<()> {
        let mut bot = self.bot.lock().await;
        let lua = &self.lua;

        loop {
            if let Some(event) = bot.next_event().await {
                self.trigger_lua_callback(&event, lua).await?;
            }

            if bot.state.joined && !bot.state.loaded_mapblocks.is_empty() {
                if let Err(e) = bot.physics_step(0.05).await {
                    tracing::info!("physics_step error: {e}");
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }
    }

    async fn trigger_lua_callback(&self, event: &Event, lua: &Lua) -> anyhow::Result<()> {
        let globals = lua.globals();

        match event {
            Event::Joined => {
                // FIX: call::<()>() instead of call::<_, ()>()
                if let Ok(func) = globals.get::<Function>("on_join") {
                    let _ = func.call::<()>(());
                }
            }

            Event::Chat { sender, text } => {
                if let Ok(func) = globals.get::<Function>("on_chat") {
                    // FIX: call::<()> with the tuple as argument
                    let _ = func.call::<()>((sender.as_str(), text.as_str()));
                }
            }

            Event::MovePlayer { pos, .. } => {
                if let Ok(func) = globals.get::<Function>("on_move") {
                    let _ = func.call::<()>((pos.x, pos.y, pos.z));
                }
            }

            Event::Hp { hp } => {
                if let Ok(func) = globals.get::<Function>("on_hp") {
                    let _ = func.call::<()>(*hp as i32);
                }
            }

            Event::Died => {
                if let Ok(func) = globals.get::<Function>("on_death") {
                    let _ = func.call::<()>(());
                }
            }

            Event::Kicked(reason) => {
                if let Ok(func) = globals.get::<Function>("on_kick") {
                    let _ = func.call::<()>(reason.as_str());
                }
            }

            Event::Disconnected => {
                if let Ok(func) = globals.get::<Function>("on_disconnect") {
                    let _ = func.call::<()>(());
                }
            }

            Event::MovementParams { walk_speed, jump_speed, gravity } => {
                if let Ok(func) = globals.get::<Function>("on_movement_params") {
                    let _ = func.call::<()>((*walk_speed, *jump_speed, *gravity));
                }
            }

            Event::BlockData { pos, .. } => {
                if let Ok(func) = globals.get::<Function>("on_block_data") {
                    let _ = func.call::<()>((pos.x as i32, pos.y as i32, pos.z as i32));
                }
            }

            _ => {}
        }

        if let Ok(func) = globals.get::<Function>("on_tick") {
            let _ = func.call::<()>(());
        }

        Ok(())
    }
}

struct BotLuaWrapper {
    bot: Arc<Mutex<Bot>>,
    state: Arc<Mutex<crate::state::BotState>>,
}

impl UserData for BotLuaWrapper {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("chat", |_, this, msg: String| {
            let bot = this.bot.clone();
            tokio::spawn(async move {
                if let Err(e) = bot.lock().await.send_chat(msg).await {
                    error!("chat error: {e}");
                }
            });
            Ok(())
        });

        methods.add_method("jump", |_, this, _: ()| {
            let mut bot = this.bot.blocking_lock();
            bot.jump();
            Ok(())
        });

        methods.add_method("stop", |_, this, _: ()| {
            let mut bot = this.bot.blocking_lock();
            bot.stop();
            Ok(())
        });

        methods.add_method("walk", |_, this, (forward, back, left, right): (bool, bool, bool, bool)| {
            let mut bot = this.bot.blocking_lock();
            bot.walk(forward, back, left, right);
            Ok(())
        });

        methods.add_method("respawn", |_, this, _: ()| {
            let bot = this.bot.clone();
            tokio::spawn(async move {
                if let Err(e) = bot.lock().await.respawn().await {
                    error!("respawn error: {e}");
                }
            });
            Ok(())
        });

        methods.add_method("disconnect", |_, this, _: ()| {
            let bot = this.bot.clone();
            tokio::spawn(async move {
                if let Err(e) = bot.lock().await.disconnect().await {
                    error!("disconnect error: {e}");
                }
            });
            Ok(())
        });

        methods.add_method("get_pos", |_, this, _: ()| {
            let state = this.state.blocking_lock();
            Ok((state.pos.x, state.pos.y, state.pos.z))
        });

        methods.add_method("get_hp", |_, this, _: ()| {
            let state = this.state.blocking_lock();
            Ok(state.hp as i32)
        });

        methods.add_method("get_username", |_, this, _: ()| {
            let bot = this.bot.blocking_lock();
            Ok(bot.username().to_string())
        });
    }
}