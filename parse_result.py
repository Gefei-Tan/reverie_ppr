import re
import csv

# List of circuit names in the same order as the output
circuit_names = ['amortized_aco_512_12.txt', 'amortized_aco_128_9.txt', 'amortized_aco_128_8.txt',
                 'amortized_aco_64_8.txt', 'amortized_aco_64_9.txt', 'amortized_aco_256_9.txt',
                 'amortized_aco_256_10.txt', 'amortized_aco_512_7.txt', 'amortized_aco_512_11.txt',
                 'amortized_aco_512_9.txt', 'amortized_aco_128_10.txt', 'amortized_aco_128_11.txt',
                 'amortized_aco_128_12.txt', 'amortized_aco_256_12.txt', 'amortized_aco_256_11.txt',
                 'amortized_aco_256_8.txt', 'amortized_aco_64_12.txt', 'amortized_aco_512_10.txt',
                 'amortized_aco_64_11.txt', 'amortized_aco_128_7.txt', 'amortized_aco_512_8.txt',
                 'amortized_aco_256_7.txt', 'amortized_aco_64_7.txt', 'amortized_aco_64_10.txt', 'amortized_akv.txt']

output_file_path = 'all_bench.txt'  # Replace with your output file path
csv_output_path = 'all_bench.csv'  # Specify the output CSV file path

with open(output_file_path, 'r') as output_file:
    lines = output_file.readlines()

parsed_data = {}

for i, line in enumerate(lines):
    if line.startswith('The size of the serialized object'):
        size = int(re.search(r'\d+', line).group())
        circuit_name = circuit_names[i]
        parsed_data[circuit_name] = {'size': size}
    elif line.startswith('Elapsed'):
        time_ms = float(re.search(r'[\d.]+', line).group())
        parsed_data[circuit_name]['time_ms'] = time_ms

# Write parsed data to CSV file
with open(csv_output_path, 'w', newline='') as csv_file:
    fieldnames = ['Circuit Name', 'Size (bytes)', 'Time (ms)']
    csv_writer = csv.DictWriter(csv_file, fieldnames=fieldnames)

    csv_writer.writeheader()
    for circuit_name, data in parsed_data.items():
        csv_writer.writerow({
            'Circuit Name': circuit_name,
            'Size (bytes)': data['size'],
            'Time (ms)': data['time_ms']
        })
