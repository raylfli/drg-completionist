use callback_future::CallbackFuture;
use fragile::Fragile;
use js_sys::JsString;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::str;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::future_to_promise;
use web_sys::{File, FileReader, ProgressEvent};

#[macro_use]
extern crate structure;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Promise<SaveFileData>")]
    pub type ParseSaveFileResult;
    #[wasm_bindgen(typescript_type = "ReadonlyMap<string, ReadonlySet<string>>")]
    pub type SaveFileOverclocks;
}

#[wasm_bindgen]
pub struct SaveFileData {
    overclocks: HashMap<String, HashSet<String>>,
}

#[wasm_bindgen]
impl SaveFileData {
    #[wasm_bindgen(getter)]
    pub fn overclocks(&self) -> SaveFileOverclocks {
        let m = js_sys::Map::new();
        for (weapon, weapon_overclocks) in &self.overclocks {
            let s = js_sys::Set::new(&JsValue::undefined());
            for overclock in weapon_overclocks {
                s.add(&JsValue::from(overclock));
            }
            m.set(&JsValue::from(weapon), &s);
        }
        JsValue::from(m).unchecked_into::<SaveFileOverclocks>()
    }
}

fn get_file_data(e: ProgressEvent) -> Option<String> {
    let target = e.target()?;
    let reader_result = JsCast::dyn_ref::<FileReader>(&target)?.result().unwrap();
    Some(JsCast::dyn_ref::<JsString>(&reader_result)?.as_string()?)
}

const SCHEMATICS_OFFSET: usize = 141;

#[derive(Deserialize)]
struct OverclockData {
    name: String,
    weapon: String,
}

fn get_file_overclocks(file: &String) -> Result<HashMap<String, HashSet<String>>, &str> {
    let file_chars: Vec<char> = file.char_indices().map(|(_, c)| c).collect();
    let pos = match file_chars
        .windows("ForgedSchematics".len())
        .position(|window| window.into_iter().collect::<String>() == "ForgedSchematics")
    {
        Some(i) => i,
        None => return Err("Failed to find start index of overclocks in file"),
    };
    let num_forged_schematics =
        match structure!("4s").unpack(file_chars[pos + 63..pos + 67].iter().collect::<String>()) {
            Ok(i) => i.0[0],
            Err(_) => return Err("Did not get a valid number of overclocks"),
        };

    let overclocks: HashMap<String, OverclockData> =
        serde_json::from_str(include_str!("guids.json")).unwrap();
    let mut acquired_overclocks: HashMap<String, HashSet<String>> = HashMap::new();
    for x in 0..num_forged_schematics {
        let start = pos + SCHEMATICS_OFFSET + ((x as i16) * 16) as usize;
        let guid = hex::encode_upper(
            file_chars[start..(start + 16)]
                .iter()
                .map(|c| *c as u8)
                .collect::<Vec<u8>>(),
        );
        if overclocks.contains_key(&guid) {
            let overclock = &overclocks[&guid];
            let weapon_overclocks = acquired_overclocks
                .entry(overclock.weapon.clone())
                .or_insert(HashSet::new());
            (*weapon_overclocks).insert(overclock.name.clone());
        }
    }

    Ok(acquired_overclocks)
}

#[wasm_bindgen(typescript_type = "Promise<SaveFileData>")]
pub fn parse_save_file(file: File) -> ParseSaveFileResult {
    let promise = future_to_promise(async {
        let file = Fragile::new(file);
        let file_data: Result<String, &str> = CallbackFuture::new(move |complete| {
            let reader = FileReader::new().unwrap();
            let onloadend_cb = Closure::once(Box::new(|e: ProgressEvent| {
                complete(match get_file_data(e) {
                    Some(d) => Ok(d),
                    None => Err("Failed to get data from file"),
                });
            }));
            reader.set_onloadend(JsCast::dyn_ref(onloadend_cb.as_ref()));
            reader
                .read_as_binary_string(&file.get())
                .expect("blob not readable");
            onloadend_cb.forget();
        })
        .await;
        match file_data {
            Ok(d) => match get_file_overclocks(&d) {
                Ok(o) => Ok(JsValue::from(SaveFileData { overclocks: o })),
                Err(e) => Err(JsValue::from(e)),
            },
            Err(e) => Err(JsValue::from(e)),
        }
    });

    JsValue::from(promise).unchecked_into::<ParseSaveFileResult>()
}
