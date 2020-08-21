'use strict';

var child_process = require('child_process');
var exec = child_process.exec;
var execSync = child_process.execSync;
var readline = require('readline').createInterface({
	input: process.stdin,
	output: process.stdout
});

function isWindows() {
	return process.platform === 'win32';
}

function installRust() {
	switch (process.platform) {
		case 'darwin':
		case 'linux':
			execSync('curl --proto \'=https\' --tlsv1.2 https://sh.rustup.rs -sSf | sh');
		case 'win32':
			execSync('start https://www.rust-lang.org/tools/install');
	}
}

// ------- REMOVE THIS WHEN SUPPORT FOR WINDOWS -----------
if (isWindows()) {
	process.stdout.write('\n' + '-'.repeat(29) + '\nWindows is not supported yet.\n' + '-'.repeat(29) + '\n\n\n');
	process.exit(1);
}
// --------------------------------------------------------

new Promise(function (res) {
	try {
		var child = exec('cargo build', { cwd: __dirname });
		child.stdout.pipe(process.stdout);
		child.stderr.pipe(process.stderr);
		child.on('close', function (code) {
			res(code);
		});
	} catch {
		res(1);
	}
}).then(code => {
	if (code !== 1) {
		process.stdout.write('\n\nBuild program did not successfully finish.\nDid you forget to install rust/cargo?\n');
		var askInstall = function () {
			readline.question('\nDo you want to install rust? (y/n) ', function (res) {
				console.log(res);
				if (res.toLowerCase().trim() === 'y') {
					installRust();
				} else if (res.toLowerCase().trim() === 'n') {
					process.exit();
				} else {
					askInstall();
				}
				readline.close();
			});
		}
		askInstall();
	}
});
