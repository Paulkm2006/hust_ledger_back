use reqwest::{Client, header};

pub async fn get_jsession(castgc: &str) -> Result<String, Box<dyn std::error::Error>> {
	let url = "http://ecard.m.hust.edu.cn/wechat-web/QueryController/Queryurl.html";
	let re_jsession = regex::Regex::new(r#"jsessionid=(.*)"#).unwrap();
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
	let jsession = match re_jsession.captures(res.url().as_str()){
		Some(caps) => match caps.get(1){
			Some(cap) => cap.as_str(),
			None => {return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "String match failed")));},
		},
		None => {return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Regex match failed. Consider refreshing the captcha.")));},
	};
	Ok(jsession.to_string())
}