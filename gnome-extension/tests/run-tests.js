// Runs the GJS tests of the modules that do not need GNOME Shell:
// gjs -m gnome-extension/tests/run-tests.js [SUITE...]

import System from 'system';

const SUITES = ['contract', 'store', 'wire', 'keys', 'spec'];

let count = 0;
for (const suite of ARGV.length ? ARGV : SUITES) {
    const module = await import(`./test-${suite}.js`);
    for (const [name, test] of Object.entries(module.tests)) {
        try {
            await test();
            print(`PASS ${suite}: ${name}`);
            count++;
        } catch (error) {
            printerr(`FAIL ${suite}: ${name}: ${error.stack}`);
            System.exit(1);
        }
    }
}
print(`${count} tests passed`);
