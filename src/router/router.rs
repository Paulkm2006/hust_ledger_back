use super::super::utils;
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.service(
		web::scope("/login")
			.route("/captcha", web::get().to(utils::captcha::get_captcha))
			.route("", web::post().to(utils::hust_login::login)),
	);
}