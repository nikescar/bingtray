package pe.nikescar.bingtray;

interface IShellCallback {
    void onOutput(String line);
    void onComplete(int exitCode);
}
