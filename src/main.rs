use libc;
use libc::{close, fork, forkpty, ioctl, setsid, winsize, TIOCGWINSZ};
use rand::Rng;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use termion::cursor::{Goto, Hide, Restore, Save, Show};
use termion::raw::{IntoRawMode, RawTerminal};

const DOG_RIGHT: [&str; 2] = [
    r"
 _      __ 
 \\____{(''o 
  (     \_` 
 /_)-/_)_) 
",
    r"
 _      __ 
 \\____{(''o 
  (     \_` 
  \_)\_)\_) 
",
];

const DOG_SITTING: &str = r"
     ___
 _  / {(''o 
 \\/   \_` 
  /_)‾\_)\_)
";
const DOG_HEIGHT: u16 = 4;
const DOG_WIDTH: usize = 12;

fn get_terminal_size(fd: libc::c_int) -> Option<(u16, u16)> {
    let mut ws: winsize = unsafe { std::mem::zeroed() };

    let result = unsafe { ioctl(fd, TIOCGWINSZ.into(), &mut ws) };

    if result == -1 {
        None
    } else {
        Some((ws.ws_col, ws.ws_row))
    }
}

fn main() {
    // Create a PTY and fork the process
    let mut master_fd: libc::c_int = 0;
    let pid = unsafe {
        forkpty(
            &mut master_fd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };

    let stdout_fd = std::io::stdout().as_raw_fd();

    // Duplicate stdout
    let saved_stdout_fd = unsafe { libc::dup(stdout_fd) };
    if saved_stdout_fd < 0 {
        eprintln!("Failed to duplicate stdout");
        std::process::exit(1);
    }

    daemonize_and_run(saved_stdout_fd);
}

fn daemonize_and_run(saved_stdout_fd: i32) {
    // Daemonize the process
    let pid = unsafe { fork() };
    if pid < 0 {
        eprintln!("Fork failed");
        std::process::exit(1);
    } else if pid > 0 {
        std::process::exit(0);
    }

    if unsafe { setsid() } < 0 {
        eprintln!("setsid failed");
        std::process::exit(1);
    }

    unsafe {
        close(libc::STDIN_FILENO);
        close(libc::STDOUT_FILENO);
        close(libc::STDERR_FILENO);
    }

    // Open the saved stdout using the duplicated file descriptor
    let saved_stdout = unsafe { File::from_raw_fd(saved_stdout_fd) };

    let raw_stdout = match saved_stdout.into_raw_mode() {
        Ok(raw) => raw,
        Err(e) => {
            eprintln!("Failed to set raw mode: {}", e);
            std::process::exit(1);
        }
    };

    let mut writer = BufWriter::new(raw_stdout);

    // Now you can write to the writer, and it should output to the terminal
    write!(writer, "Writer is now writing correctly.\n").unwrap();
    writer.flush().unwrap();

    loop {
        // Walk the dog
        walk_dog(saved_stdout_fd, &mut writer);

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn start_write<W: Write>(writer: &mut W) -> std::io::Result<()> {
    write!(writer, "{}", Save)?;
    write!(writer, "{}", Hide)?;
    Ok(())
}

fn end_write<W: Write>(writer: &mut W) -> std::io::Result<()> {
    write!(writer, "{}", Restore)?;
    write!(writer, "{}", Show)?;
    writer.flush()?;
    Ok(())
}

fn write_multi_line_message_from_position<W: Write>(
    writer: &mut W,
    x: u16,
    y: u16,
    msg: &[&str],
) -> std::io::Result<()> {
    for (i, line) in msg.iter().enumerate() {
        let current_y = y + i as u16;
        write!(writer, "{}", Goto(x, current_y))?;
        write!(writer, "{}", line)?;
    }
    Ok(())
}

// Rest of your code...

fn walk_dog(saved_stdout_fd: libc::c_int, writer: &mut BufWriter<RawTerminal<File>>) {
    let (width, height) = match get_terminal_size(saved_stdout_fd) {
        Some((cols, rows)) => (cols, rows),
        None => {
            eprintln!("Failed to get terminal size");
            return;
        }
    };

    // Pick a random starting height
    let mut rng = rand::thread_rng();
    let start_y = rng.gen_range(1..=((height - DOG_HEIGHT - 2) as u16));

    let mut position: (i32, i32) = (1, start_y as i32);
    let mut previous_position: Option<(i32, i32)> = None;

    let mut walk_state = 0;
    let mut has_pooped = false;
    loop {
        if position.0 > width as i32 {
            break;
        }

        start_write(writer).unwrap();

        if let Some(prev_position) = previous_position {
            clear_area(
                writer,
                prev_position.0.max(1) as u16,
                (prev_position.1 + 1).max(1) as u16,
                DOG_WIDTH as u16,
                DOG_HEIGHT as u16 - 1,
            )
            .unwrap();
        }

        // Draw the dog at the new position
        {
            let dog = DOG_RIGHT[walk_state];
            let dog_lines = if width as usize - position.0 as usize > DOG_WIDTH {
                dog.to_string()
            } else if position.0 < 0 {
                let max_length = width as usize - position.0.abs() as usize;
                trim_lines_to_length_from_end(dog.to_string(), max_length)
            } else {
                let max_length = width as usize - position.0 as usize;

                trim_lines_to_length(dog.to_string(), max_length)
            };

            write_multi_line_message_from_position(
                writer,
                position.0.max(1) as u16,
                position.1.max(1) as u16,
                &dog_lines.lines().collect::<Vec<_>>(),
                // &mut *buffer,
            )
            .unwrap();
        }

        end_write(writer).unwrap();

        previous_position = Some(position);
        position = (position.0 + 1, position.1);
        if position.0 % 4 == 0 {
            walk_state = (walk_state + 1) % 2;
        }

        if position.0 > (width).div_ceil(2) as i32 && !has_pooped {
            start_write(writer).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(500));

            write_multi_line_message_from_position(
                writer,
                position.0 as u16,
                position.1 as u16,
                &DOG_SITTING.lines().collect::<Vec<_>>(),
            )
            .unwrap();
            end_write(writer).unwrap();

            // sleep for a bit
            std::thread::sleep(std::time::Duration::from_millis(2500));
            has_pooped = true;
            write_multi_line_message_from_position(
                writer,
                position.0 as u16 + 2,
                position.1 as u16 + DOG_HEIGHT as u16 + 1,
                &["💩"],
            )
            .unwrap();
            end_write(writer).unwrap();

            std::thread::sleep(std::time::Duration::from_millis(500));

            // write_multi_line_message_from_position(
            //     writer,
            //     position.0.max(1) as u16,
            //     position.1.max(1) as u16,
            //     &["💩"],
            // )
            // .unwrap();
            // end_write(writer).unwrap();
            // has_pooped = true;
        }

        // Sleep for a bit
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    end_write(writer).unwrap();

    return;
}

fn clear_area<W: Write>(
    writer: &mut W,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> std::io::Result<()> {
    for i in 0..height {
        for j in 0..width {
            write!(writer, "{}", Goto(x + j, y + i))?;
            write!(writer, " ")?;
        }
    }

    Ok(())
}

fn trim_lines_to_length(lines: String, length: usize) -> String {
    lines
        .lines()
        .map(|line| {
            if line.len() > length {
                line.chars().take(length).collect::<String>()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn trim_lines_to_length_from_end(lines: String, length: usize) -> String {
    lines
        .lines()
        .map(|line| {
            if line.len() > length {
                line.chars().rev().take(length).collect::<String>()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}
