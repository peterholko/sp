/*use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "cmd")]
enum NetworkPacket {
    #[serde(rename = "login")]
    Login { account_name: String, password: String },
    #[serde(rename = "select_class")]
    SelectedClass { class_name: String },
    #[serde(rename = "move_unit")]
    Move { x: i32, y: i32 },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "packet")]
enum ResponsePacket {
    #[serde(rename = "select_class")]
    SelectClass {
        player: u32,
    },
    #[serde(rename = "info_select_class")]
    InfoSelectClass {
        result: String,
    },
    PlayerMoved {
        player_id: i32,
        x: i32,
        y: i32,
    },
    Ok,
    Error {
        errmsg: String,
    },
}*/

