#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/epoll.h>
#include <unistd.h>

/* Isolated Linux contract probes; this is not a Bream backend. */
#define REQUIRE(expr) do { if (!(expr)) { \
    fprintf(stderr, "%s:%d: %s (errno=%d)\n", __FILE__, __LINE__, #expr, errno); \
    exit(1); } } while (0)

static int create_poll(void) {
    int fd = epoll_create1(EPOLL_CLOEXEC);
    REQUIRE(fd >= 0);
    return fd;
}
static void make_pipe(int p[2]) { REQUIRE(pipe2(p, O_NONBLOCK | O_CLOEXEC) == 0); }
static void ctl(int poll, int op, int fd, uint32_t events, uint64_t token) {
    struct epoll_event ev = {.events = events, .data.u64 = token};
    REQUIRE(epoll_ctl(poll, op, fd, &ev) == 0);
}
static int wait_one(int poll, struct epoll_event *ev, int ms) {
    int n;
    do { n = epoll_wait(poll, ev, 1, ms); } while (n < 0 && errno == EINTR);
    REQUIRE(n >= 0);
    return n;
}
static void close_fd(int fd) { REQUIRE(close(fd) == 0); }

static void copied_event_reuse(void) {
    int poll = create_poll(), old[2], next[2];
    struct epoll_event copied, fresh;
    make_pipe(old);
    make_pipe(next); /* Reserve the replacement before releasing the old number. */
    ctl(poll, EPOLL_CTL_ADD, old[0], EPOLLIN, 101);
    REQUIRE(write(old[1], "a", 1) == 1);
    REQUIRE(wait_one(poll, &copied, 1000) == 1);
    REQUIRE(copied.data.u64 == 101 && (copied.events & EPOLLIN));
    ctl(poll, EPOLL_CTL_DEL, old[0], 0, 0);
    int reused = old[0];
    close_fd(old[0]);
    REQUIRE(dup2(next[0], reused) == reused);
    close_fd(next[0]);
    ctl(poll, EPOLL_CTL_ADD, reused, EPOLLIN, 202);
    REQUIRE(write(next[1], "b", 1) == 1);
    REQUIRE(wait_one(poll, &fresh, 1000) == 1);
    REQUIRE(copied.data.u64 == 101 && fresh.data.u64 == 202);
    char byte;
    REQUIRE(read(reused, &byte, 1) == 1 && byte == 'b');
    close_fd(reused); close_fd(next[1]); close_fd(old[1]); close_fd(poll);
    puts("{\"case\":\"copied_event_reuse\",\"old_token\":101,\"new_token\":202,\"same_fd_reused\":true,\"read_from_reused_fd\":\"b\"}");
}

static void duplicate_lifetime(void) {
    int poll = create_poll(), p[2];
    struct epoll_event ev;
    make_pipe(p);
    int other = dup(p[0]);
    REQUIRE(other >= 0);
    ctl(poll, EPOLL_CTL_ADD, p[0], EPOLLIN, 303);
    close_fd(p[0]);
    REQUIRE(write(p[1], "c", 1) == 1);
    REQUIRE(wait_one(poll, &ev, 1000) == 1 && ev.data.u64 == 303);
    errno = 0;
    REQUIRE(epoll_ctl(poll, EPOLL_CTL_DEL, other, NULL) == -1 && errno == ENOENT);
    close_fd(other);
    REQUIRE(wait_one(poll, &ev, 0) == 0);
    close_fd(p[1]); close_fd(poll);
    puts("{\"case\":\"duplicate_lifetime\",\"event_after_original_close\":true,\"delete_via_duplicate\":\"ENOENT\",\"empty_after_last_close\":true}");
}

static void consumed_readiness(void) {
    int poll = create_poll(), p[2];
    struct epoll_event ev;
    make_pipe(p);
    ctl(poll, EPOLL_CTL_ADD, p[0], EPOLLIN, 404);
    REQUIRE(write(p[1], "d", 1) == 1);
    REQUIRE(wait_one(poll, &ev, 1000) == 1 && (ev.events & EPOLLIN));
    char byte;
    REQUIRE(read(p[0], &byte, 1) == 1 && byte == 'd');
    errno = 0;
    REQUIRE(read(p[0], &byte, 1) == -1 && (errno == EAGAIN || errno == EWOULDBLOCK));
    close_fd(p[0]); close_fd(p[1]); close_fd(poll);
    puts("{\"case\":\"consumed_readiness\",\"readiness_observed\":true,\"second_read\":\"EAGAIN\"}");
}

static void oneshot_rearm(void) {
    int poll = create_poll(), p[2];
    struct epoll_event ev;
    make_pipe(p);
    ctl(poll, EPOLL_CTL_ADD, p[0], EPOLLIN | EPOLLONESHOT, 505);
    REQUIRE(write(p[1], "e", 1) == 1);
    REQUIRE(wait_one(poll, &ev, 1000) == 1 && ev.data.u64 == 505);
    REQUIRE(wait_one(poll, &ev, 0) == 0);
    ctl(poll, EPOLL_CTL_MOD, p[0], EPOLLIN | EPOLLONESHOT, 506);
    REQUIRE(wait_one(poll, &ev, 1000) == 1 && ev.data.u64 == 506);
    close_fd(p[0]); close_fd(p[1]); close_fd(poll);
    puts("{\"case\":\"oneshot_rearm\",\"disabled_before_mod\":true,\"ready_after_mod\":true,\"rearmed_token\":506}");
}

static void hup_with_data(void) {
    int poll = create_poll(), p[2];
    struct epoll_event ev;
    make_pipe(p);
    ctl(poll, EPOLL_CTL_ADD, p[0], EPOLLIN, 606);
    REQUIRE(write(p[1], "f", 1) == 1);
    close_fd(p[1]);
    REQUIRE(wait_one(poll, &ev, 1000) == 1);
    REQUIRE((ev.events & EPOLLIN) && (ev.events & EPOLLHUP));
    char byte;
    REQUIRE(read(p[0], &byte, 1) == 1 && byte == 'f');
    REQUIRE(read(p[0], &byte, 1) == 0);
    close_fd(p[0]); close_fd(poll);
    puts("{\"case\":\"hup_with_data\",\"readable_and_hup\":true,\"data_before_eof\":true}");
}

int main(int argc, char **argv) {
    REQUIRE(argc == 2);
    if (!strcmp(argv[1], "copied_event_reuse")) copied_event_reuse();
    else if (!strcmp(argv[1], "duplicate_lifetime")) duplicate_lifetime();
    else if (!strcmp(argv[1], "consumed_readiness")) consumed_readiness();
    else if (!strcmp(argv[1], "oneshot_rearm")) oneshot_rearm();
    else if (!strcmp(argv[1], "hup_with_data")) hup_with_data();
    else REQUIRE(0);
    return 0;
}
