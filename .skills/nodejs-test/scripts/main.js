const readline = require('readline');

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false
});

let inputData = '';

rl.on('line', (line) => {
    inputData += line;
});

rl.on('close', () => {
    try {
        const input = JSON.parse(inputData);
        const result = {
            success: true,
            original: input.text,
            processed: input.text.toUpperCase()
        };
        console.log(JSON.stringify(result));
    } catch (error) {
        console.log(JSON.stringify({
            success: false,
            error: error.message
        }));
    }
});
