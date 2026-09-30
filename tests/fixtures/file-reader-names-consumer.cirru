
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|通过普通 Calcit 模块引用验证同名函数，不调用 core 文件读取") (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ [] |./file-reader-names-module.cirru
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc "|通过模块别名调用普通函数，与 core 的真实文件读取区分。")
          :code $ quote $ defn main! ()
            assert= |reader:file:example $ readers/read-file |example
            assert= |reader:dir:example $ readers/read-dir |example
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ []
            %{} 'TestEntry (:name |qualified-module-file-reader)
              :code $ quote $ assert= |reader:file:example (readers/read-file |example)
            %{} 'TestEntry (:name |qualified-module-directory-reader)
              :code $ quote $ assert= |reader:dir:example (readers/read-dir |example)
        'reload! $ %{} 'CodeEntry (:doc "|开发模式重载占位入口。")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
          :require $ fix-command.reader :as readers
