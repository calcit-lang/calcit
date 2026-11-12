
{}
  :package |demo
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'demo.main/main!) (:mode :native) (:reload-fn 'demo.main/main!)
      :feature-policy $ {} (:io :error)
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'demo.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println |ran)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'CodeEntry (:doc |)
        :code $ quote $ ns demo.main
        :examples $ []
        :schema nil
