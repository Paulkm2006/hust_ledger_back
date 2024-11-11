use regex::Regex;
use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use actix_web::{HttpResponse, Responder};
use reqwest::Client;

#[derive(Serialize)]
pub struct Captcha {
	status: i32,
	msg: String,
	ticket: String,
	jsessionid: String,
	img_base64: String,
}

pub async fn get_captcha() -> Result<impl Responder, Box<dyn std::error::Error>> {
	let re_ticket = Regex::new(r#"<input type="hidden" id="lt" name="lt" value="(.*)" />"#).unwrap();
	let request_url = "https://pass.hust.edu.cn/cas/login";
	let captcha_url = "https://pass.hust.edu.cn/cas/code";

	
	let client = Client::builder()
		.cookie_store(true)
		.build()?;
	let res = match client.get(request_url).send().await{
		Ok(res) => res,
		Err(e) => {
			return Ok(HttpResponse::InternalServerError().json(Captcha {
				status: 500,
				msg: format!("Failed to get token: {}", e),
				ticket: "".to_string(),
				jsessionid: "".to_string(),
				img_base64: "".to_string(),
			}));
		}
	};
	let jsession = res.cookies().find(|c| c.name() == "JSESSIONID").unwrap().value().to_string();
	let body = res.text().await.unwrap();
	let ticket = re_ticket.captures(&body).unwrap().get(1).unwrap().as_str();

	let captcha = match client.get(captcha_url).send().await{
		Ok(res) => res,
		Err(e) => {
			return Ok(HttpResponse::InternalServerError().json(Captcha {
				status: 500,
				msg: format!("Failed to get CAPTCHA: {}", e),
				ticket: "".to_string(),
				jsessionid: "".to_string(),
				img_base64: "".to_string(),
			}));
		}
	};
	let img = captcha.bytes().await.unwrap();
	let img_base64 = general_purpose::STANDARD.encode(img.as_ref());


	Ok(HttpResponse::Ok().json(Captcha {
		status: 200,
		msg: "Success".to_string(),
		ticket: ticket.to_string(),
		jsessionid: jsession,
		img_base64: img_base64,
	}))
}