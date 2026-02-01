const { loadPyodide } = require("pyodide");
const fs = require("fs");

async function main() {
    let inputData = "";
    
    const chunks = [];
    process.stdin.on("data", chunk => chunks.push(chunk));
    
    await new Promise(resolve => process.stdin.on("end", resolve));
    inputData = Buffer.concat(chunks).toString();
    
    try {
        const pyodide = await loadPyodide();
        
        // Read Python code file path (from environment variable)
        const pythonCodePath = process.env.PYTHON_CODE_PATH;
        const pythonCode = fs.readFileSync(pythonCodePath, "utf8");
        
        // Setup stdin - use string concatenation to avoid template string issues
        const stdinSetupCode = [
            "import sys",
            "import io",
            "",
            "class StdinWrapper:",
            "    def __init__(self, data):",
            "        self._buffer = io.StringIO(data)",
            "    def read(self):",
            "        return self._buffer.read()",
            "    def readline(self):",
            "        return self._buffer.readline()",
            "",
            "sys.stdin = StdinWrapper(" + JSON.stringify(inputData) + ")"
        ].join("\n");
        
        pyodide.runPython(stdinSetupCode);
        
        // Capture stdout
        pyodide.runPython([
            "import sys",
            "import io",
            "_stdout_buffer = io.StringIO()",
            "sys.stdout = _stdout_buffer"
        ].join("\n"));
        
        // Execute user code
        pyodide.runPython(pythonCode);
        
        // Get output
        const output = pyodide.runPython("_stdout_buffer.getvalue()");
        console.log(output);
        
    } catch (error) {
        console.error("Pyodide error:", error.message);
        process.exit(1);
    }
}

main();
