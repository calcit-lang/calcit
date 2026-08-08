
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |app) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |app.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (test-valid-list-methods) (; test-invalid-list-method ; "会导致" preprocess "错误") (; test-invalid-string-method ; "会导致" preprocess "错误") (test-invalid-map-method ; "测试" map "方法验证") (println |All tests passed)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn reload! $
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
        |test-invalid-list-method $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-list-method () $ let
                xs $ [] 1 2 3
              assert-type xs 'List
              ; "非法：list" "没有" invalid-method
              .invalid-method xs
          :examples $ []
          :schema $ :: 'Dynamic
        |test-invalid-map-method $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-map-method () $ let
                m $ {} (:a 1)
              assert-type m 'Map
              ; "非法：map" "没有" invalid-map-method
              .invalid-map-method m
          :examples $ []
          :schema $ :: 'Dynamic
        |test-invalid-string-method $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-string-method () $ let
                text |hello
              assert-type text 'String
              ; "非法：string" "没有" invalid-string-method
              .invalid-string-method text
          :examples $ []
          :schema $ :: 'Dynamic
        |test-valid-list-methods $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-valid-list-methods () $ let
                xs $ [] 1 2 3
              assert-type xs 'List
              ; "合法的" list "方法"
              .first xs
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote (ns app.main)
