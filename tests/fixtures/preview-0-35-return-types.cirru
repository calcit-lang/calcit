
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |preview
  :entries $ {} $ :default
    {} (:description "|0.35.0 返回类型预告提示 fixture（#2564）") (:init-fn 'preview.core/main!) (:mode :native) (:reload-fn 'preview.core/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'preview.core
    %{} 'FileEntry
      :defs $ {}
        'count-rows $ %{} 'CodeEntry (:doc "|接受任意列表。")
          :code $ quote $ defn count-rows (xs) (count xs)
          :examples $ []
          :schema $ quote $ Fn (List Dynamic) (-> Number)
        'main! $ %{} 'CodeEntry
          :doc "|把 diff-triple 结果整体传给 List 参数在 0.35.0 会报错；按位置读取和 Cirru 行列表传给 List Dynamic 仍然合法。"
          :code $ quote $ defn main! ()
            let
                triple $ &map:diff-triple
                  {} $ |a 1
                  {} $ |b 2
                rows $ parse-cirru-list "|a b"
              println (count-rows triple) (&list:nth triple 0) (count triple) (count-rows rows)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns preview.core
