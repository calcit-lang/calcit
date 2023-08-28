
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ {}
      :configs $ {}
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ + 1 2
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns app.main $ :require
      :proc $ quote ()
