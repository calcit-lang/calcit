
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |js-match-destruct-list
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'js-match-destruct-list.main/main!) (:mode :native) (:reload-fn 'js-match-destruct-list.main/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'js-match-destruct-list.main
    %{} 'FileEntry
      :defs $ {}
        'first-or $ %{} 'CodeEntry (:doc "|Fused: (:some) first with a `_` tail binder.")
          :code $ quote $ defn first-or (xs)
            match (destruct-list xs)
              (:some h _) h
              (:none) 0
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
        'head-plus-one $ %{} 'CodeEntry (:doc "|Fused in expression position.")
          :code $ quote $ defn head-plus-one (xs)
            &+ 1 $ match (destruct-list xs)
              (:none) 0
              (:some h _) h
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ sum-list $ [] 1 2 3
            println $ first-or $ []
            println $ rest-size $ [] 1 2
            println $ head-plus-one $ [] 4
            println $ shadowed-subject $ [] 5
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'rest-size $ %{} 'CodeEntry (:doc "|Fused: `_` head binder.")
          :code $ quote $ defn rest-size (xs)
            match (destruct-list xs)
              (:none) -1
              (:some _ t) (&list:count t)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
        'shadowed-subject $ %{} 'CodeEntry
          :doc "|Generic path: the head binder shadows the subject."
          :code $ quote $ defn shadowed-subject (xs)
            match (destruct-list xs)
              (:none) 0
              (:some xs _) xs
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
        'sum-list $ %{} 'CodeEntry
          :doc "|Fused: (:none) first, recursive call in the (:some) branch."
          :code $ quote $ defn sum-list (xs)
            match (destruct-list xs)
              (:none) 0
              (:some h t)
                &+ h $ sum-list t
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] $ :: 'List 'Number
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns js-match-destruct-list.main (:require)
