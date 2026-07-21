fn main() {
    // 记录编译时间作为版本号
    let now = chrono::Local::now();
    println!("cargo:rustc-env=BUILD_TIME={}", now.format("%Y%m%d%H%M%S"));
}
