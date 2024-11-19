use super::super::utils;
use super::super::controller;
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.service(
	web::scope("/login")
			.route("/captcha", web::get().to(utils::captcha::get_captcha))
			.route("", web::post().to(utils::hust_login::login))
			.route("/refresh", web::get().to(utils::hust_login::refresh_jsession)));
	cfg.service(
		web::scope("/report").route("", web::get().to(controller::report::report))
	);
}