use axum::{extract::Path, http::StatusCode, response::Html};

// Public landing page hit by the browser after Safepay's hosted checkout
// redirects the customer. Purely presentational until a real frontend exists.
pub async fn payment_result(
    Path((order_id, result)): Path<(String, String)>,
) -> (StatusCode, Html<String>) {
    let (title, message) = match result.as_str() {
        "success" => (
            "Payment received".to_string(),
            format!(
                "Your payment for order {} was received. We will email you once it ships.",
                order_id
            ),
        ),
        "cancelled" => (
            "Payment cancelled".to_string(),
            format!(
                "Payment for order {} was cancelled. No charge was made.",
                order_id
            ),
        ),
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Invalid result</title></head><body><h1>Invalid payment result</h1><p>Unrecognized result '{}'.</p></body></html>",
                    other
                )),
            );
        }
    };

    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;display:flex;align-items:center;\
         justify-content:center;height:100vh;margin:0}}div{{text-align:center}}h1{{margin-bottom:.5rem}}</style>\
         </head><body><div><h1>{}</h1><p>{}</p></div></body></html>",
        title, title, message
    );

    (StatusCode::OK, Html(html))
}
