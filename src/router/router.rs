use super::super::utils;
use super::super::controller;
use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.service(
	web::scope("/login")
			.route("", web::post().to(utils::hust_login::login)));
	cfg.service(
		web::scope("/report").route("", web::get().to(controller::report::report))
	);
	cfg.service(
		web::scope("/tags")
			.route("", web::get().to(controller::tags::get_tags))
	);
}