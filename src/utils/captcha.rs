use regex::Regex;
use super::ocr;
use reqwest::Client;


pub async fn get_captcha() -> Result<(String, String, String), Box<dyn std::error::Error>> {
	let re_ticket = Regex::new(r#"<input type="hidden" id="lt" name="lt" value="(.*)" />"#).unwrap();
	let request_url = "https://pass.hust.edu.cn/cas/login";
	let captcha_url = "https://pass.hust.edu.cn/cas/code";

	let client = Client::builder()
		.cookie_store(true)
		.build()?;
	let res = client.get(request_url).send().await?;
	let jsession = res.cookies().find(|c| c.name() == "JSESSIONID").unwrap().value().to_string();
	let body = res.text().await.unwrap();
	let ticket = re_ticket.captures(&body).unwrap().get(1).unwrap().as_str();

	let captcha = client.get(captcha_url).send().await?;
	let img = captcha.bytes().await.unwrap();
	let code = ocr::decode_captcha(img).await?;
	Ok((ticket.to_string(), jsession, code))
}