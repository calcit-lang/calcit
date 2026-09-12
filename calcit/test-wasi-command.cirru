
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |app)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!) (:target :wasm)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.main $ %{} 'FileEntry
      :defs $ {}
        'clock-fixed-main! $ %{} 'CodeEntry (:doc "|用确定性的 WASI 假宿主验证时钟编号与纳秒到毫秒的换算。")
          :code $ quote
            defn clock-fixed-main! () $ if
              and
                = 1234 $ unix-time-ms
                = 5678 $ cpu-time
              println "|WASI-fixed-clocks: ok"
              quit! 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :time :wasi
        'clock-main! $ %{} 'CodeEntry (:doc "|输出系统时钟与单调时钟，用于验证 WASI capability wiring。")
          :code $ quote
            defn clock-main! () $ if (clocks-valid?) (println "|WASI-clocks: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :time :wasi
        'clocks-valid? $ %{} 'CodeEntry (:doc "|验证系统时钟为正数，且单调时钟在同一进程内不会倒退。")
          :code $ quote
            defn clocks-valid? () $ let
                wall $ unix-time-ms
                before $ cpu-time
                after $ cpu-time
              and (number? wall) (> wall 0) (number? before) (number? after) (>= after before)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ []
          :tags $ #{} :core :time :unit :wasi :wasm
          :tests $ []
            %{} 'TestEntry (:name |valid-readings)
              :code $ quote
                assert= true $ clocks-valid?
              :tags $ #{} :core :time :unit :wasi :wasm
        'exit-7! $ %{} 'CodeEntry (:doc "|以状态码 7 终止进程，用于验证 command 退出边界。")
          :code $ quote
            defn exit-7! () $ quit! 7
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :control :wasi
        'exit-invalid! $ %{} 'CodeEntry (:doc "|以越界状态码终止进程，用于验证各 command backend 拒绝隐式取模。")
          :code $ quote
            defn exit-invalid! () $ quit! 256
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :control :wasi
        'filesystem-absolute-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn filesystem-absolute-main! () $ if (filesystem-read-error? |/workspace/input.txt) (println "|WASI-filesystem-absolute: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'filesystem-denied-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn filesystem-denied-main! () $ if (filesystem-read-error? |workspace/input.txt) (println "|WASI-filesystem-denied: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'filesystem-invalid-utf8-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn filesystem-invalid-utf8-main! () $ if (filesystem-read-error? |workspace/invalid.txt) (println "|WASI-filesystem-invalid-utf8: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'filesystem-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn filesystem-main! () $ let
                input $ .read-text (fs:path |workspace/input.txt)
                output $ .write-text (fs:path |workspace/output.txt) "|WASI-written: 好"
                input-ok? $ match input
                  (:ok content) (= content "|WASI-file: 你好")
                  (:err _) false
                output-ok? $ match output
                  (:ok _) true
                  (:err _) false
              if (and input-ok? output-ok?) (println "|WASI-filesystem: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'filesystem-read-dir-error-main! $ %{} 'CodeEntry (:doc "|通过 WASI 假宿主验证畸形目录记录仍返回 Result :err。")
          :code $ quote
            defn filesystem-read-dir-error-main! () $ if (filesystem-read-dir-error? |workspace/listing) (println "|WASI-read-dir-error: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :file :wasi
        'filesystem-read-dir-error? $ %{} 'CodeEntry (:doc "|验证目录枚举失败仍以 Result :err 表达。")
          :code $ quote
            defn filesystem-read-dir-error? (path)
              match
                .read-dir $ fs:path path
                (:ok _) false
                (:err _) true
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String
          :tags $ #{} :file :unit :wasi
          :tests $ []
            %{} 'TestEntry (:name |missing-directory)
              :code $ quote
                assert= true $ filesystem-read-dir-error? |/calcit-wasi-filesystem-does-not-exist
              :tags $ #{} :file :unit :wasi
        'filesystem-read-dir-main! $ %{} 'CodeEntry (:doc "|通过真实与假 WASI 宿主验证目录分页、UTF-8 名称和确定排序。")
          :code $ quote
            defn filesystem-read-dir-main! () $ match
              .read-dir $ fs:path |workspace/listing
              (:ok paths)
                if
                  and
                    = 3 $ count paths
                    = |workspace/listing/a.txt $ &struct:nth (&list:nth paths 0) 0 :value
                    = |workspace/listing/b.txt $ &struct:nth (&list:nth paths 1) 0 :value
                    = "|workspace/listing/子.txt" $ &struct:nth (&list:nth paths 2) 0 :value
                  println "|WASI-read-dir: ok"
                  quit! 1
              (:err _) (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :file :wasi
        'filesystem-read-error? $ %{} 'CodeEntry (:doc "|验证 FsPath 文本读取失败仍以 Result :err 表达，不把 host error 泄漏为异常。")
          :code $ quote
            defn filesystem-read-error? (path)
              match
                .read-text $ fs:path path
                (:ok _) false
                (:err _) true
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'String
          :tests $ []
            %{} 'TestEntry (:name |missing-path)
              :code $ quote
                assert= true $ filesystem-read-error? |/calcit-wasi-filesystem-does-not-exist/input.txt
              :tags $ #{} :file :unit :wasi
        'filesystem-traversal-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn filesystem-traversal-main! () $ if (filesystem-read-error? |workspace/../secret.txt) (println "|WASI-filesystem-traversal: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (println |WASI-stdout: "|你好") (echo |WASI-echo) (eprintln |WASI-stderr: 42)
              println |WASI-env: $ option:unwrap-or (get-env |CALCIT_WASI_TEST_ENV) |missing
              each (get-args)
                fn (arg) (println |WASI-arg: arg)
              + 1 2
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
          :tests $ []
            %{} 'TestEntry (:name |returns-three)
              :code $ quote
                assert= 3 $ main!
              :tags $ #{} :core :unit :wasi :wasm
        'needs-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn needs-arg (x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
          :tests $ []
            %{} 'TestEntry (:name |returns-input)
              :code $ quote
                assert= 3 $ needs-arg 3
              :tags $ #{} :core :unit :wasi :wasm
        'random-error? $ %{} 'CodeEntry (:doc "|验证越界的安全随机请求返回 String 错误。")
          :code $ quote
            defn random-error? (size)
              match (secure-random-bytes size)
                (:ok _) false
                (:err message) (string? message)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number
          :tags $ #{} :core :crypto :unit :wasi :wasm
          :tests $ []
            %{} 'TestEntry (:name |rejects-negative-length)
              :code $ quote
                assert= true $ random-error? -1
              :tags $ #{} :core :crypto :unit :wasi :wasm
            %{} 'TestEntry (:name |rejects-oversized-length)
              :code $ quote
                assert= true $ random-error? 65537
              :tags $ #{} :core :crypto :unit :wasi :wasm
            %{} 'TestEntry (:name |rejects-fractional-length)
              :code $ quote
                assert= true $ random-error? 1.5
              :tags $ #{} :core :crypto :unit :wasi :wasm
        'random-fixed-main! $ %{} 'CodeEntry (:doc "|由确定性 WASI 假宿主验证四字节随机请求。")
          :code $ quote
            defn random-fixed-main! () $ if (random-success? 4) (println "|secure-random-fixed: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :crypto :wasi
        'random-main! $ %{} 'CodeEntry (:doc "|验证安全随机 API 的成功与越界 Result 语义。")
          :code $ quote
            defn random-main! () $ if
              and (random-success? 16) (random-error? 65537)
              println "|secure-random: ok"
              quit! 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :crypto :wasi
        'random-success? $ %{} 'CodeEntry (:doc "|验证指定长度的安全随机请求返回 Buffer。")
          :code $ quote
            defn random-success? (size)
              match (secure-random-bytes size)
                (:ok bytes) (buffer? bytes)
                (:err _) false
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number
          :tags $ #{} :core :crypto :unit :wasi :wasm
          :tests $ []
            %{} 'TestEntry (:name |returns-empty-buffer)
              :code $ quote
                assert= true $ random-success? 0
              :tags $ #{} :core :crypto :unit :wasi :wasm
            %{} 'TestEntry (:name |returns-buffer)
              :code $ quote
                assert= true $ random-success? 16
              :tags $ #{} :core :crypto :unit :wasi :wasm
        'read-args $ %{} 'CodeEntry (:doc "|读取当前宿主进程的完整参数列表。")
          :code $ quote
            defn read-args () $ get-args
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
              :features $ #{} :env :io
              :return $ :: 'List 'String
          :tests $ []
            %{} 'TestEntry (:name |returns-strings)
              :code $ quote
                assert= true $ every? (read-args) string?
              :tags $ #{} :core :env :unit :wasi :wasm
        'read-clocks $ %{} 'CodeEntry (:doc "|读取系统时钟与单调时钟的毫秒值。")
          :code $ quote
            defn read-clocks () $ [] (unix-time-ms) (cpu-time)
          :examples $ []
          :schema $ :: 'Fn
            {}
              :args $ []
              :return $ :: 'List 'Number
          :tags $ #{} :time :wasi
          :tests $ []
            %{} 'TestEntry (:name |returns-numbers)
              :code $ quote
                assert= true $ every? (read-clocks) number?
              :tags $ #{} :core :time :unit :wasi :wasm
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        'wait-error? $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn wait-error? (duration)
              match (wait-ms duration)
                (:ok _) false
                (:err message) (string? message)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number
          :tags $ #{} :core :io :time :unit :wasi
          :tests $ []
            %{} 'TestEntry (:name |rejects-negative)
              :code $ quote
                assert= true $ wait-error? -1
              :tags $ #{} :core :time :unit :wasi
            %{} 'TestEntry (:name |rejects-fractional)
              :code $ quote
                assert= true $ wait-error? 1.5
              :tags $ #{} :core :time :unit :wasi
            %{} 'TestEntry (:name |rejects-overflow)
              :code $ quote
                assert= true $ wait-error? 4294967296
              :tags $ #{} :core :time :unit :wasi
        'wait-failure-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn wait-failure-main! () $ if (wait-error? 1) (println "|WASI-wait-error: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :io :time :wasi
        'wait-fixed-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn wait-fixed-main! () $ if
              and (wait-success? 0) (wait-success? 4294967295)
              println "|WASI-fixed-wait: ok"
              quit! 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :io :time :wasi
        'wait-main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn wait-main! () $ if (wait-success? 1) (println "|WASI-wait: ok") (quit! 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
          :tags $ #{} :io :time :wasi
        'wait-success? $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn wait-success? (duration)
              match (wait-ms duration)
                (:ok _) true
                (:err _) false
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Bool)
              :args $ [] 'Number
          :tags $ #{} :core :io :time :unit :wasi
          :tests $ []
            %{} 'TestEntry (:name |zero-succeeds)
              :code $ quote
                assert= true $ wait-success? 0
              :tags $ #{} :core :time :unit :wasi
            %{} 'TestEntry (:name |positive-succeeds)
              :code $ quote
                assert= true $ wait-success? 1
              :tags $ #{} :core :time :unit :wasi
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns app.main $ :require
