{}
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'Cell $ %{} 'CodeEntry (:doc "|An Option member does not admit a bare nil.")
          :code $ quote $ defunion Cell String
            quote $ Option Number
          :examples $ []
        'f $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn f (v)
            match v
              (String s) s
              (Option o) |opt
          :examples $ []
          :schema $ quote $ Fn Cell (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println (f nil)
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
