
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native)
      :reload-fn 'app.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'get-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn get-number () 'Number (do 123)
          :examples $ []
          :schema $ :: 'Dynamic
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (test-type-info)
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! ()
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-type-info $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-type-info ()
            let
                x 123
              ; "这里" assert-type "会把类型信息加到" scope_types
              assert-type x 'Number
              ; "后续使用" x "时，Local" "节点会读取" scope_types "中的类型信息"
              let
                  y $ &+ x 1
                  z |hello
                  flag true
                  nothing nil
                  nums $ [] 1 2 3
                  result $ get-number
                  ; "测试嵌套表达式：内层" &let "的返回值类型"
                  nested $ let
                      inner 456
                    inner
                println y
                println z
                println flag
                println nothing
                println nums
                println result
                println nested
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
