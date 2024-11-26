use actix_web::{web, HttpResponse, Responder};
use reqwest::{Client, header};
use serde::{Serialize, Deserialize};
use serde_json;
use base64::{engine::general_purpose, Engine};
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use rand;
use rsa::pkcs8::DecodePublicKey;

#[derive(Deserialize)]
pub struct Credential {
	username: String,
	password: String,
}


#[derive(Serialize)]
struct Info{
	status: i32,
	msg: String,
	castgc: String,
}

pub async fn get_account_no(castgc: &str) -> Result<String, Box<dyn std::error::Error>>{
	let url = "http://ecard.m.hust.edu.cn/wechat-web/QueryController/Queryurl.html";
	let regex = regex::Regex::new(r#"<input id="account" type="hidden" value="(.*)"/>"#).unwrap();

	let cookie_store = reqwest::cookie::Jar::default();
	let url_token = reqwest::Url::parse("https://pass.hust.edu.cn").unwrap();
	cookie_store.add_cookie_str(("CASTGC=".to_owned() + &castgc).as_str(), &url_token);

	let client = Client::builder()
	.cookie_provider(cookie_store.into())
	.default_headers({
		let mut headers = header::HeaderMap::new();
		headers.insert(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3".parse().unwrap());
		headers.insert(header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".parse().unwrap());
		headers.insert(header::ACCEPT_ENCODING, "gzip, deflate, sdch".parse().unwrap());
		headers.insert(header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.8".parse().unwrap());
		headers.insert(header::CONNECTION, "keep-alive".parse().unwrap());
		headers
	})
	.build()
	.unwrap();

	let res = client.get(url).send().await?;
	let resp = res.text().await.unwrap();
	let account_no = match regex.captures(resp.as_str()){
		Some(caps) => match caps.get(1){
			Some(cap) => cap.as_str(),
			None => {return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "String match failed")));},
		},
		None => {return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Login expired.")));},
	};
	Ok(account_no.to_string())
}

pub async fn login(cred: web::Json<Credential>) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let (lt, jsession, code) = match super::captcha::get_captcha().await{
		Ok((lt, jsession, code)) => (lt, jsession, code),
		Err(e) => return Ok(HttpResponse::InternalServerError().json(Info{
			status: 500,
			msg: format!("Failed to get captcha: {}", e),
			castgc: "".to_string(),
		})),
	};
	let cookie_store = reqwest::cookie::Jar::default();
	let url = reqwest::Url::parse("https://pass.hust.edu.cn").unwrap();
	cookie_store.add_cookie_str(("JSESSIONID=".to_owned() + &jsession).as_str(), &url);
	let mut client = Client::builder()
		.redirect(reqwest::redirect::Policy::none())
		.cookie_provider(cookie_store.into())
		.default_headers({
			let mut headers = header::HeaderMap::new();
			headers.insert(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3".parse().unwrap());
			headers.insert(header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".parse().unwrap());
			headers.insert(header::ACCEPT_ENCODING, "gzip, deflate, sdch".parse().unwrap());
			headers.insert(header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.8".parse().unwrap());
			headers.insert(header::CONNECTION, "keep-alive".parse().unwrap());
			headers
		})
		.build()?;
	let castgc = match get_castgc(cred, lt, code, &mut client).await{
		Ok(castgc) => castgc,
		Err(e) => return Ok(HttpResponse::Forbidden().json(Info{
			status: 403,
			msg: format!("Failed to login: {}", e),
			castgc: "".to_string(),
		})),
	};
	Ok(HttpResponse::Ok().json(Info{
		status: 200,
		msg: "Login successful.".to_string(),
		castgc: castgc,
	}))
}


async fn get_castgc(cred: web::Json<Credential>,lt: String, code: String, client: &mut Client) -> Result<String, Box<dyn std::error::Error>> {
	let url = "https://pass.hust.edu.cn/cas/login?service=http%3A%2F%2Fecard.m.hust.edu.cn%3A80%2Fwechat-web%2FQueryController%2Fselect.html";
	let rsa_url = "https://pass.hust.edu.cn/cas/rsa";

	let rsa_res = client.post(rsa_url).send().await?;
	let rsa_json: serde_json::Value = serde_json::from_str(&rsa_res.text().await.unwrap()).unwrap();
	let rsa_b64 = rsa_json["publicKey"].as_str().unwrap();
	let rsa_key = general_purpose::STANDARD.decode(rsa_b64.as_bytes()).unwrap();
	let rsa = RsaPublicKey::from_public_key_der(&rsa_key).unwrap();
	let mut rng = rand::thread_rng();

	let ul_vec = rsa.encrypt(&mut rng, Pkcs1v15Encrypt, cred.username.as_bytes()).unwrap();
	let ul = general_purpose::STANDARD.encode(ul_vec.as_ref() as &[u8]);
	let pl_vec = rsa.encrypt(&mut rng, Pkcs1v15Encrypt, cred.password.as_bytes()).unwrap();
	let pl = general_purpose::STANDARD.encode(pl_vec.as_ref() as &[u8]);

	let res = client.post(url)
		.form(&[("ul", ul), ("pl", pl), ("lt", lt), ("code", code[..4].to_string()), 
				("rsa", "".to_string()), ("phoneCode", "".to_string()), ("execution", "e1s1".to_string()), 
				("_eventId", "submit".to_string())])
		.send().await?;
	let cookies = res.cookies().collect::<Vec<_>>();
	match cookies.iter().find(|c| c.name() == "CASTGC"){
		Some(t) => Ok(t.value().to_string()),
		None => Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Wrong username, password or captcha."))),
	}

}
