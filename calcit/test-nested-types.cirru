
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
        'compute $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn compute (x) 'Number (&+ x 10)
          :examples $ []
          :schema $ :: 'Dynamic
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ test-nested-scope
          :examples $ []
          :schema $ :: 'Dynamic
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! ()
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'test-nested-scope $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn test-nested-scope ()
            ; "测试：外层定义的变量可以被内层使用，并保留类型信息"
            let
                a 100
              let
                  ; b "使用外层的" "a，a" "的类型信息应该传递进来"
                  b $ &+ a 20
                let
                    ; c "使用中层的" b "和外层的" a
                    c $ &+ b a
                    ; d "调用函数，函数的返回类型应该推断出来"
                    d $ compute c
                  ; "最终返回" "d，类型应该是" :number
                  d
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main
