use bevy::prelude::*;
use std::{char::MAX, collections::HashMap};

use serde::{Deserialize, Serialize};

use crate::{
    constants::MAX_PRICE,
    item::{Item, ItemSubclass},
    obj::Position,
    templates::PriceTemplates,
};

#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct Prices(pub HashMap<String, Price>);

#[derive(Debug, Reflect, Clone)]
pub struct Price {
    pub name: String,
    pub buy_price: i32, // The price the merchant is buying at
    pub buy_quantity: i32,
    pub sell_price: i32, // The price the merchant is selling at
    pub sell_quantity: i32,
    pub impact_factor: f32,
}

impl Prices {
    pub fn load_from_template(&mut self, price_templates: PriceTemplates) {
        for (item_name, price_template) in price_templates.0.iter() {
            let price = Price {
                name: item_name.to_string(),
                buy_price: price_template.buy_price,
                buy_quantity: price_template.buy_quantity,
                sell_price: price_template.sell_price,
                sell_quantity: price_template.sell_quantity,
                impact_factor: price_template.impact_factor,
            };

            self.0.insert(item_name.to_string(), price);
        }
    }

    pub fn get_sell_price(&self, item_name: String) -> Option<i32> {
        if let Some(price) = self.0.get(&item_name) {
            return Some(price.sell_price);
        } else {
            return None;
        }
    }

    pub fn get_sell_quantity(&self, item_name: String) -> Option<i32> {
        if let Some(price) = self.0.get(&item_name) {
            return Some(price.sell_quantity);
        } else {
            return None;
        }
    }

    pub fn get_buy_price(&self, item_name: String) -> Option<i32> {
        if let Some(price) = self.0.get(&item_name) {
            return Some(price.buy_price);
        } else {
            return None;
        }
    }

    pub fn find_buy_price(&self, name: String, subclass: String, class: String) -> Option<i32> {
        debug!(
            "Finding buy price for: {:?}, {:?}, {:?}",
            name, subclass, class
        );
        debug!("Prices: {:?}", self.0);
        if let Some(price) = self.0.get(&name) {
            return Some(price.buy_price);
        } else if let Some(price) = self.0.get(&subclass) {
            return Some(price.buy_price);
        } else if let Some(price) = self.0.get(&class) {
            return Some(price.buy_price);
        } else {
            return None;
        }
    }

    pub fn find_sell_price(&self, name: String, subclass: String, class: String) -> Option<i32> {
        if let Some(price) = self.0.get(&name) {
            return Some(price.sell_price);
        } else if let Some(price) = self.0.get(&subclass) {
            return Some(price.sell_price);
        } else if let Some(price) = self.0.get(&class) {
            return Some(price.sell_price);
        } else {
            return None;
        }
    }

    pub fn get_buy_quantity(&self, item_name: String) -> Option<i32> {
        if let Some(price) = self.0.get(&item_name) {
            return Some(price.buy_quantity);
        } else {
            return None;
        }
    }

    pub fn adjust_sell_price(&mut self, identifier: String, quantity: i32) {
        if let Some(price) = self.0.get_mut(&identifier) {
            let quantity_ratio = quantity as f32 / price.sell_quantity as f32;
            let new_price = price.sell_price as f32 * (1.0 + quantity_ratio * price.impact_factor);

            price.sell_price = new_price as i32;
            price.sell_quantity -= quantity;

            debug!("Adjusted sell price: {:?}", price);
        }
    }

    pub fn adjust_buy_price(&mut self, identifier: String, quantity: i32) {
        if let Some(price) = self.0.get_mut(&identifier) {
            let quantity_ratio = quantity as f32 / (price.buy_quantity as f32 + quantity as f32);

            let new_price = price.buy_price as f32 * (1.0 - quantity_ratio * price.impact_factor);

            price.buy_price = new_price as i32;
            price.buy_quantity -= quantity;

            debug!("Adjusted buy price: {:?}", price);
        }
    }
}

#[derive(Resource, Reflect, Default, Debug)]
#[reflect(Resource)]
pub struct TradePorts(pub HashMap<String, TradePort>);

#[derive(Debug, Reflect, Clone)]
pub struct TradePort {
    pub name: String,
    pub empire: String,
    pub pos: Position,
    pub wanted_items: Vec<WantedItem>,
}

#[derive(Debug, Reflect, Clone, Deserialize, Serialize, PartialEq)]
pub struct WantedItem {
    pub name: Option<String>,
    pub subclass: Option<String>,
    pub class: Option<String>,
    pub quantity: i32,
    pub price: i32,
}

impl WantedItem {
    pub fn new_by_name(name: String) -> WantedItem {
        WantedItem {
            name: Some(name),
            subclass: None,
            class: None,
            quantity: -1,
            price: -1,
        }
    }

    pub fn new_by_subclass(subclass: String) -> WantedItem {
        WantedItem {
            name: None,
            subclass: Some(subclass),
            class: None,
            quantity: -1,
            price: -1,
        }
    }

    pub fn new_by_class(class: String) -> WantedItem {
        WantedItem {
            name: None,
            subclass: None,
            class: Some(class),
            quantity: -1,
            price: -1,
        }
    }

    pub fn get_identifier(&self) -> String {
        if let Some(name) = &self.name {
            return name.to_string();
        }

        if let Some(subclass) = &self.subclass {
            return subclass.to_string();
        }

        if let Some(class) = &self.class {
            return class.to_string();
        }

        return "".to_string();
    }
}

/*impl TradePorts {
    pub fn new() -> TradePorts {
        TradePorts(HashMap::new())
    }

    pub fn create_trade_port(&mut self, name: String, empire: String, pos: Position) {
        let wanted_copper_ore = WantedItem {
            name: None,
            subclass: Some("Copper Ore".to_string()),
            class: None,
            quantity: 100,
            price: 20,
            expiry: 1000,
        };

        let wanted_maple_wood = WantedItem {
            name: None,
            subclass: Some("Maple Timber".to_string()),
            class: None,
            quantity: 100,
            price: 20,
            expiry: 1000,
        };

        let wanted_items = vec![wanted_copper_ore, wanted_maple_wood];

        let trade_port = TradePort {
            name: name.clone(),
            empire,
            pos,
            wanted_items: wanted_items
        };

        self.0.insert(name, trade_port);
    }

    pub fn get_trade_port(&self, name: String) -> Option<&TradePort> {
        self.0.get(&name)
    }
}*/

/*#[derive(Debug, Reflect, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeGood {
    pub subclass: String,
    pub sell_price: i32,
    pub quantity_to_sell: i32,
    pub buy_price: i32,
    pub quantity_to_buy: i32,
}

impl TradePort {
    pub fn new(name: String, empire: String, pos: Position) -> TradePort {

        let trade_good = TradeGood {
            subclass: ItemSubclass::CopperOre,
            sell_price: 20,
            quantity_to_sell: 100,
            buy_price: 10,
            quantity_to_buy: 100,
        };


        TradePort {
            name,
            empire,
            pos,
            trade_goods: HashMap::new(),
        }
    }

    pub fn add_trade_good(&mut self, trade_good: TradeGood) {
        self.trade_goods
            .insert(trade_good.subclass.clone(), trade_good);
    }

    pub fn get_trade_good(&self, subclass: String) -> Option<&TradeGood> {
        self.trade_goods.get(String)
    }
}*/
