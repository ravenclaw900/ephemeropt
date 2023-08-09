use ephemeropt::EphemeralOption;

fn main() {
    let mut num_opt = EphemeralOption::new(0, std::time::Duration::from_secs(1));
    loop {
        match num_opt.get() {
            Some(&num) => println!("{num}"),
            None => {
                let prev_num = num_opt.get_expired().unwrap();
                let num = num_opt.insert(prev_num + 1);
                println!("{num}");
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
