use std::collections::HashMap;
use std::sync::mpsc;

use rand::thread_rng;
use rand::seq::SliceRandom;
use rand::Rng;
use std::{thread, time};

use crate::goose::{GooseTaskSet, GooseClient, GooseClientMode, GooseClientCommand};

pub fn client_main(
    thread_number: usize,
    thread_task_set: GooseTaskSet,
    mut thread_client: GooseClient,
    thread_receiver: mpsc::Receiver<GooseClientCommand>,
    thread_sender: mpsc::Sender<GooseClient>,
) {
    info!("launching client {} from {}...", thread_number, thread_task_set.name);
    // Notify parent that our run mode has changed to Running.
    thread_client.set_mode(GooseClientMode::RUNNING);
    thread_sender.send(thread_client.clone()).unwrap();

    // Client is starting, first invoke the weighted on_start tasks.
    if thread_client.weighted_on_start_tasks.len() > 0 {
        for mut sequence in thread_client.weighted_on_start_tasks.clone() {
            if sequence.len() > 1 {
                sequence.shuffle(&mut thread_rng());
            }
            for task_index in &sequence {
                // Determine which task we're going to run next.
                let thread_task_name = &thread_task_set.tasks[*task_index].name;
                let function = thread_task_set.tasks[*task_index].function.expect(&format!("{} {} missing load testing function", thread_task_set.name, thread_task_name));
                debug!("launching on_start {} task from {}", thread_task_name, thread_task_set.name);
                // Invoke the task function.
                function(&mut thread_client);
            }
        }
    }

    // Repeatedly loop through all available tasks in a random order.
    let mut thread_continue = true;
    while thread_continue {
        // Weighted_tasks is divided into buckets of tasks sorted by sequence, and then all non-sequenced tasks.
        if thread_client.weighted_tasks[thread_client.weighted_bucket].len() <= thread_client.weighted_bucket_position {
            // This bucket is exhausted, move on to position 0 of the next bucket.
            thread_client.weighted_bucket_position = 0;
            thread_client.weighted_bucket += 1;
            if thread_client.weighted_tasks.len() <= thread_client.weighted_bucket {
                thread_client.weighted_bucket = 0;
            }
            // Shuffle new bucket before we walk through the tasks.
            thread_client.weighted_tasks[thread_client.weighted_bucket].shuffle(&mut thread_rng());
            debug!("re-shuffled {} tasks: {:?}", &thread_task_set.name, thread_client.weighted_tasks[thread_client.weighted_bucket]);
        }

        // Determine which task we're going to run next.
        let thread_weighted_task = thread_client.weighted_tasks[thread_client.weighted_bucket][thread_client.weighted_bucket_position];
        let thread_task_name = &thread_task_set.tasks[thread_weighted_task].name;
        let function = thread_task_set.tasks[thread_weighted_task].function.expect(&format!("{} {} missing load testing function", thread_task_set.name, thread_task_name));
        debug!("launching {} task from {}", thread_task_name, thread_task_set.name);
        // If task name is set, it will be used for storing request statistics instead of the raw url.
        thread_client.request_name = thread_task_name.clone();
        // Invoke the task function.
        function(&mut thread_client);

        // Move to the next task in thread_client.weighted_tasks.
        thread_client.weighted_bucket_position += 1;

        // Check if the parent thread has sent us any messages.
        let message = thread_receiver.try_recv();
        if message.is_ok() {
            match message.unwrap() {
                // Sync our state to the parent.
                GooseClientCommand::SYNC => {
                    thread_sender.send(thread_client.clone()).unwrap();
                    // Reset per-thread counters, as totals have been sent to the parent
                    thread_client.requests = HashMap::new();
                },
                // Sync our state to the parent and then exit.
                GooseClientCommand::EXIT => {
                    thread_client.set_mode(GooseClientMode::EXITING);
                    thread_sender.send(thread_client.clone()).unwrap();
                    // No need to reset per-thread counters, we're exiting and memory will be freed
                    thread_continue = false
                }
            }
        }

        if thread_client.min_wait > 0 {
            let wait_time = rand::thread_rng().gen_range(thread_client.min_wait, thread_client.max_wait);
            let sleep_duration = time::Duration::from_secs(wait_time as u64);
            debug!("client {} from {} sleeping {:?} seconds...", thread_number, thread_task_set.name, sleep_duration);
            thread::sleep(sleep_duration);
        }
    }

    // Client is exiting, first invoke the weighted on_stop tasks.
    if thread_client.weighted_on_stop_tasks.len() > 0 {
        for mut sequence in thread_client.weighted_on_stop_tasks.clone() {
            if sequence.len() > 1 {
                sequence.shuffle(&mut thread_rng());
            }
            for task_index in &sequence {
                // Determine which task we're going to run next.
                let thread_task_name = &thread_task_set.tasks[*task_index].name;
                let function = thread_task_set.tasks[*task_index].function.expect(&format!("{} {} missing load testing function", thread_task_set.name, thread_task_name));
                debug!("launching on_stop {} task from {}", thread_task_name, thread_task_set.name);
                // Invoke the task function.
                function(&mut thread_client);
            }
        }
    }

}