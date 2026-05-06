// SPDX-License-Identifier: MPL-2.0

#include "../../common/test.h"

#include <pty.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

static int master, slave;

FN_SETUP(openpty)
{
	CHECK(openpty(&master, &slave, NULL, NULL, NULL));
}
END_SETUP()

FN_TEST(blocking_slave_write_completes_large_buffer)
{
	enum {
		large_write_len = 16 * 1024,
		chunk_len = 1024,
	};

	char *write_buf = CHECK(malloc(large_write_len));
	memset(write_buf, 'a', large_write_len);

	pid_t child = TEST_SUCC(fork());
	if (child == 0) {
		size_t total_read = 0;
		char read_buf[chunk_len];

		TEST_SUCC(close(slave));

		while (total_read < large_write_len) {
			ssize_t len = read(master, read_buf, sizeof(read_buf));
			if (len <= 0) {
				_exit(EXIT_FAILURE);
			}

			for (ssize_t i = 0; i < len; i++) {
				if (read_buf[i] != 'a') {
					_exit(EXIT_FAILURE);
				}
			}

			total_read += len;
		}

		TEST_SUCC(close(master));

		_exit(EXIT_SUCCESS);
	}

	TEST_SUCC(close(master));
	TEST_RES(write(slave, write_buf, large_write_len),
		 _ret == large_write_len);
	TEST_SUCC(close(slave));

	int status = 0;
	TEST_RES(waitpid(child, &status, 0),
		 _ret == child && WIFEXITED(status) &&
			 WEXITSTATUS(status) == EXIT_SUCCESS);

	free(write_buf);
}
END_TEST()
