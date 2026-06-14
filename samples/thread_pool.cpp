#include <condition_variable>
#include <functional>
#include <mutex>
#include <queue>
#include <thread>
#include <vector>

// a fixed-size thread pool that runs queued jobs and joins on destruction.
class ThreadPool {
public:
    explicit ThreadPool(std::size_t n) {
        for (std::size_t i = 0; i < n; ++i)
            workers_.emplace_back([this] { worker(); });
    }

    ~ThreadPool() {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            stop_ = true;
        }
        cv_.notify_all();
        for (auto& w : workers_) w.join();
    }

    void submit(std::function<void()> job) {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            jobs_.push(std::move(job));
        }
        cv_.notify_one();
    }

private:
    void worker() {
        for (;;) {
            std::function<void()> job;
            {
                std::unique_lock<std::mutex> lock(mutex_);
                cv_.wait(lock, [this] { return stop_ || !jobs_.empty(); });
                if (stop_ && jobs_.empty()) return;
                job = std::move(jobs_.front());
                jobs_.pop();
            }
            job();
        }
    }

    std::vector<std::thread> workers_;
    std::queue<std::function<void()>> jobs_;
    std::mutex mutex_;
    std::condition_variable cv_;
    bool stop_ = false;
};
