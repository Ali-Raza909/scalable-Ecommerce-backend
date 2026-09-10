use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CartItem {
    pub product_id: String,
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cart {
    pub items: Vec<CartItem>,
}

#[derive(Debug, Deserialize)]
pub struct AddItemRequest {
    pub product_id: String,
    pub quantity: i32,
}