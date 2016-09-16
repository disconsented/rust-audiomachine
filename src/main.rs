extern crate discord;

use std::path::Path;
use std::fs;
use std::env;
use std::fmt;
use std::time::Duration;
use std::thread;
use discord::{Discord, State};
use discord::model::Event;

const PREFIX: char = '$';

fn main(){
    let discord = Discord::from_bot_token(&env::var("DISCORD_TOKEN").expect("Bad DISCORD_TOKEN")).expect("Login failed");
    let audio_dir = "/root/Desktop/sounds/";

    // establish websocket and voice connection
    let (mut connection, ready) = discord.connect().expect("connect failed");
    println!("[Ready] {} is serving {} servers", ready.user.username, ready.servers.len());

    let mut state = State::new(ready);
    loop {
        let event = match connection.recv_event() {
            Ok(event) => event,
            Err(err) => {
                println!("[Warning] Receive error: {:?}", err);
                if let discord::Error::WebSocket(..) = err {
                    // Handle the websocket connection being dropped
                    let (new_connection, ready) = discord.connect().expect("connect failed");
                    connection = new_connection;
                    state = State::new(ready);
                    println!("[Ready] Reconnected successfully.");
                }
                if let discord::Error::Closed(..) = err {
                    break
                }
                continue
            },
        };
        state.update(&event);

        match event {
            Event::MessageCreate(message) => {
                let message = message;
                //use std::ascii::AsciiExt;
                // safeguard: stop if the message is from us
                if message.author.id == state.user().id{
                    continue
                }

                // reply to a command if there was one
                let mut split = message.content.split(' ');
                let first_word = split.next().unwrap_or("");
                let argument = split.next().unwrap_or("");
                let prefix = first_word.chars().nth(0).unwrap();
                let command: String = first_word.chars().collect::<Vec<_>>()[1..].to_vec().into_iter().collect();
                println!("{} {} {}", prefix, command, argument);
                println!("Received {:?} from { }", message.content, message.author.name);

                if prefix == PREFIX {
                    let vchan = state.find_voice_user(message.author.id);
                    match command.as_ref() {
                        "stop" =>{
                            vchan.map(|(sid, _)| connection.voice(sid).stop());
                        },
                        "sleep" => {
                            thread::sleep(Duration::from_millis(4000));
                        }
                        "list" =>{
                            let paths = fs::read_dir(audio_dir).unwrap();
                            let mut clips: String = String::from("");
                            for path in paths {
                                let unwrapped_path = path.unwrap().path();
                                let clip_slice: &str = &format!("{:?}, ", unwrapped_path.file_name().unwrap());
                                clips.push_str(clip_slice);
                            }
                            let result = discord.send_message(&message.channel_id, clips.as_ref(), "",  false);
                        },
                        "play" => {
                            let vchan = state.find_voice_user(message.author.id);
                            let output = if let Some((server_id, channel_id)) = vchan {
                                let mut owned_dir: String = audio_dir.to_owned();
                                let owned_arguement: String = argument.to_owned();
                                owned_dir.push_str(&owned_arguement);
                                match discord::voice::open_ffmpeg_stream(Path::new(&owned_dir)) {
                                    Ok(stream) => {
                                        println!("Joining {:?}", server_id);
                                        let voice = connection.voice(server_id);
                                        voice.set_deaf(true);
                                        voice.connect(channel_id);
                                        voice.play(stream);
                                        String::new()
                                    },
                                    Err(error) => format!("Error: {}", error),
                                }
                            } else {
                                "You must be in a voice channel to DJ".to_owned()
                            };
                            if output.is_empty() {
                                warn(discord.send_message(&message.channel_id, &output, "", false));
                            }
                        },
                        _ => continue
                    }
                }
            }

            Event::VoiceStateUpdate(server_id, _) => {
                // If someone moves/hangs up, and we are in a voice channel,
                if let Some(cur_channel) = connection.voice(server_id).current_channel() {
                    // and our current voice channel is empty, disconnect from voice
                    if let Some(srv) = state.servers().iter().find(|srv| srv.id == server_id) {
                        if srv.voice_states.iter().filter(|vs| vs.channel_id == Some(cur_channel)).count() <= 1 {
                            connection.voice(server_id).disconnect();
                        }
                    }
                }
            }
            _ => {}, // discard other events
        }
    }
}

fn warn<T, E: ::std::fmt::Debug>(result: Result<T, E>) {
    match result {
        Ok(_) => {},
        Err(err) => println!("[Warning] {:?}", err)
    }
}