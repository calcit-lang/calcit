# Published WASI preflight errors / 发布版 WASI 调用前错误

## 中文

- 明确区分 guest typed 结果与参数文件解析等调用前 host 错误；后者只通过 stderr 和稳定退出码表达，不创建虚假的 guest 结果文件。
- 发布版业务 smoke 捕获并断言调用前错误的规范诊断，同时在退出码不符时输出捕获的 stdout 与 stderr。

## English

- Distinguish typed guest results from pre-invocation host failures such as argument-file decoding; the latter use stderr and a stable exit code without fabricating a guest result file.
- Capture and assert the canonical pre-invocation diagnostic in the published business smoke, and print captured stdout and stderr when an exit code is unexpected.
