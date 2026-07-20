
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (test-valid-list-methods) (; test-invalid-list-method ; "会导致" preprocess "错误") (; test-invalid-string-method ; "会导致" preprocess "错误") (test-invalid-map-method ; "测试" map "方法验证") (println |All tests passed)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () nil
          :examples $ []
        |test-invalid-list-method $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-invalid-list-method () $ let
                xs $ [] 1 2 3
              assert-type xs :list
              ; "非法：list" "没有" invalid-method
              .invalid-method xs
          :examples $ []
        |test-invalid-map-method $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-invalid-map-method () $ let
                m $ {} (:a 1)
              assert-type m :map
              ; "非法：map" "没有" invalid-map-method
              .invalid-map-method m
          :examples $ []
        |test-invalid-string-method $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-invalid-string-method () $ let
                text |hello
              assert-type text :string
              ; "非法：string" "没有" invalid-string-method
              .invalid-string-method text
          :examples $ []
        |test-valid-list-methods $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn test-valid-list-methods () $ let
                xs $ [] 1 2 3
              assert-type xs :list
              ; "合法的" list "方法"
              .first xs
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns app.main)
