use iced_widget::runtime::Task;
use iced_widget::runtime::futures::futures::channel::mpsc;
use iced_widget::runtime::futures::futures::channel::oneshot;
use iced_widget::runtime::futures::futures::stream::{self, StreamExt};

pub fn try_spawn_blocking<T, E>(
    f: impl FnOnce(mpsc::Sender<T>) -> Result<(), E> + Send + 'static,
) -> Task<Result<T, E>>
where
    T: Send + 'static,
    E: Send + 'static,
{
    let (sender, receiver) = mpsc::channel(1);
    let (error_sender, error_receiver) = oneshot::channel();

    let _ = std::thread::spawn(move || {
        if let Err(error) = f(sender) {
            let _ = error_sender.send(Err(error));
        }
    });

    Task::stream(stream::select(
        receiver.map(Ok),
        stream::once(error_receiver).filter_map(async |result| result.ok()),
    ))
}
